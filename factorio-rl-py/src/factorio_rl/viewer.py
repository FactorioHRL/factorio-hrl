"""Live viewer: a tiny local web page that shows the latest layout while a search or training runs.

``Viewer.publish`` writes the renderer's PNG, the ASCII grid, the blueprint string and the score
history to a directory that a stdlib HTTP server serves; the page polls it every second.
Open http://localhost:8000 (``Viewer.start`` opens the browser itself).
"""

from __future__ import annotations

import json
import threading
import webbrowser
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from .engine import Engine
from .scoring import Weights, breakdown
from .task import Placement, Task, ascii_grid

PAGE = """<!doctype html>
<html><head><meta charset="utf-8"><title>factorio-rl live</title>
<style>
 body{font:14px system-ui,sans-serif;margin:0;background:#1d1f23;color:#e6e6e6;display:flex;gap:16px;padding:16px;flex-wrap:wrap}
 .card{background:#2a2d33;border-radius:8px;padding:12px}
 img{image-rendering:pixelated;max-width:min(60vw,900px);max-height:80vh;background:#222}
 pre{margin:0;font:13px ui-monospace,monospace}
 textarea{width:100%;height:5em;background:#1d1f23;color:#9fd;border:0;font:11px ui-monospace,monospace}
 canvas{background:#1d1f23;width:100%;height:140px}
 h3{margin:0 0 8px} .k{color:#9aa} .good{color:#7fdc7f}
</style></head><body>
<div class="card"><h3 id="title">waiting for the first layout...</h3><img id="img" alt="layout"></div>
<div style="display:flex;flex-direction:column;gap:16px;min-width:340px;max-width:460px">
 <div class="card"><h3>Status</h3><pre id="stats"></pre></div>
 <div class="card"><h3>Score history</h3><canvas id="chart" width="420" height="140"></canvas></div>
 <div class="card"><h3>Grid (M = machine)</h3><pre id="ascii"></pre></div>
 <div class="card"><h3>Blueprint (paste into the game)</h3><textarea id="bp" readonly></textarea></div>
</div>
<script>
let last=-1;
async function tick(){
  try{
    const s=await (await fetch('state.json?'+Date.now())).json();
    if(s.version!==last){ last=s.version;
      document.getElementById('img').src='latest.png?'+s.version;
      document.getElementById('title').textContent=s.title;
      document.getElementById('stats').textContent=s.stats;
      document.getElementById('ascii').textContent=s.ascii;
      document.getElementById('bp').value=s.blueprint;
      draw(s.history);
    }
  }catch(e){}
  setTimeout(tick,1000);
}
function draw(h){
  const c=document.getElementById('chart'),g=c.getContext('2d');g.clearRect(0,0,c.width,c.height);
  if(!h.length)return;
  const all=h.flatMap(p=>[p[1],p[2]]).filter(v=>v!==null);
  const lo=Math.min(0,...all),hi=Math.max(1,...all);
  const X=i=>10+i*(c.width-20)/Math.max(1,h.length-1),Y=v=>c.height-10-(v-lo)/(hi-lo)*(c.height-20);
  const line=(k,color,dash)=>{
    g.strokeStyle=color;g.setLineDash(dash);g.beginPath();let pen=false;
    h.forEach((p,i)=>{if(p[k]===null){return;}pen?g.lineTo(X(i),Y(p[k])):g.moveTo(X(i),Y(p[k]));pen=true;});
    g.stroke();g.setLineDash([]);
  };
  g.strokeStyle='#445';g.beginPath();g.moveTo(10,Y(1));g.lineTo(c.width-10,Y(1));g.stroke();
  line(1,'#6cf',[3,3]);line(2,'#fc6',[]);
  g.font='11px sans-serif';
  g.fillStyle='#6cf';g.fillText('dashed: the layout shown on the left',14,12);
  g.fillStyle='#fc6';g.fillText('solid: mean over all evaluation tasks',14,26);
}
tick();
</script></body></html>
"""


class Viewer:
    """Publishes layouts to a directory and serves it."""

    def __init__(
        self,
        engine: Engine,
        out_dir: str | Path = "live",
        port: int = 8000,
        game_dir: str | Path | None = None,
        scale: int = 32,
        weights: Weights | None = None,
    ) -> None:
        self.engine, self.game_dir, self.scale, self.port, self.weights = engine, game_dir, scale, port, weights
        self.out = Path(out_dir)
        self.out.mkdir(parents=True, exist_ok=True)
        (self.out / "index.html").write_text(PAGE)
        self._history: list[tuple[int, float, float | None]] = []
        self._version = 0
        self._server: ThreadingHTTPServer | None = None

    def start(self, open_browser: bool = True) -> str:
        """Serves the directory in a background thread and returns the URL."""
        handler = partial(_QuietHandler, directory=str(self.out))
        self._server = ThreadingHTTPServer(("127.0.0.1", self.port), handler)
        threading.Thread(target=self._server.serve_forever, daemon=True).start()
        url = f"http://localhost:{self.port}"
        if open_browser:
            webbrowser.open(url)
        print(f"live viewer: {url}")
        return url

    def stop(self) -> None:
        if self._server:
            self._server.shutdown()

    def publish(
        self,
        task: Task,
        layout: list[Placement],
        label: str,
        step: int,
        score: float,
        report: dict,
        reference: float | None = None,
        mean_score: float | None = None,
    ) -> None:
        """Shows ``layout`` (the renderer's picture, grid text, blueprint). The chart gets two series:
        ``score`` of this layout and, when given, ``mean_score`` over all evaluation tasks."""
        if layout:
            (self.out / "latest.png").write_bytes(
                self.engine.render_png(layout, self.game_dir, self.scale)
            )
        self._history.append((step, score, mean_score))
        self._version += 1
        stats = format_stats(task, step, score, report, reference, mean_score, breakdown(report, self.weights))
        state = {
            "version": self._version,
            "title": f"{label}: {task.width}x{task.height}",
            "stats": stats,
            "ascii": ascii_grid(task.width, task.height, report["placed"]),
            "blueprint": self.engine.export_blueprint(layout),
            "history": self._history[-300:],
            "reference": reference,
        }
        tmp = self.out / "state.json.tmp"
        tmp.write_text(json.dumps(state))
        tmp.replace(self.out / "state.json")  # atomic: the page never reads a half-written file


def format_stats(
    task: Task,
    step: int,
    score: float,
    report: dict,
    reference: float | None = None,
    mean_score: float | None = None,
    terms: dict[str, float] | None = None,
) -> str:
    """The status panel as plain text: what the layout makes, what it eats and how the score is built."""
    target = task.target
    lines = [f"Grid {task.width}x{task.height}   step {step}"]
    lines.append(f"Score {score:.3f}" + (f"  (reference {reference:.3f})" if reference is not None else ""))
    if mean_score is not None:
        lines.append(f"  (this layout is one evaluation task; mean over all of them {mean_score:.3f})")
    for name, value in (terms or {}).items():  # what each reward term adds to (+) or takes from (-) the score
        lines.append(f"  {value:+.3f}  {name}")
    lines += ["", f"PRODUCES  {target}"]
    ceiling = report["ceiling_per_second"]
    share = report["output_per_second"] / ceiling if ceiling > 1e-9 else 0.0
    limit = "the required" if task.required_output is not None else "at most"
    lines.append(f"  {report['output_per_second']:.2f}/s  of {limit} {ceiling:.2f}/s  ({share:.0%})")
    surplus = [f for f in report["flows"] if f["per_second"] > 1e-9 and f["item"] != target]
    lines += [f"  left over: {f['item']} {f['per_second']:.2f}/s" for f in surplus]

    lines += ["", "CONSUMES  (per second, from the supply)"]
    drawn = {f["item"]: -f["per_second"] for f in report["flows"] if f["per_second"] < -1e-9}
    for item, rate in task.supply.items():
        used = drawn.pop(item, 0.0)
        limit = "unlimited" if rate is None else f"of {rate:g}/s ({used / rate:.0%})" if rate > 0 else "of 0/s"
        lines.append(f"  {item:<22} {used:>7.2f}  {limit}")
    lines += [f"  {item:<22} {used:>7.2f}  not in the supply!" for item, used in drawn.items()]

    lines += ["", f"MACHINES  {report['machines']} placed, {report['tiles_used']}/{report['tiles_total']} tiles used"]
    for g in report["groups"]:
        lines.append(f"  {g['count']:>3} x {g['recipe']:<22} busy {g['utilisation']:.0%}")
    if report["idle_machines"] > 0.05:
        lines.append(f"  idle capacity: {report['idle_machines']:.1f} machines' worth")
    return "\n".join(lines)


class _QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, *args) -> None:  # keep the training log readable
        pass

    def handle(self) -> None:
        try:
            super().handle()
        except ConnectionError:  # the browser closed the tab mid-request
            pass
