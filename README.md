# factorio-hrl

Narzędzia do blueprintów Factorio (dekodowanie, model, render, generator). Specyfikacja w `SPEC.md`.

## Wymagane dane (nie ma ich w repo)

Grafiki i dump pochodzą od Wube, więc **trzeba mieć własną kopię gry** i skopiować z niej dane do `data/` (katalog jest w `.gitignore`):

```
scripts/setup-data.sh "<katalog gry>" <data-raw-dump.json>
# WSL + Steam, przykład:
scripts/setup-data.sh "/mnt/c/Program Files (x86)/Steam/steamapps/common/Factorio" /mnt/c/temp/fsim-dump21/script-output/data-raw-dump.json
```

Skrypt kopiuje `graphics/entity` i `graphics/icons` każdego moda (ok. 1,8 GB, kilka minut przez `/mnt/c`) do `data/game/data/<mod>/` oraz dump do `data/data-raw-dump.json`.

Dump musi pochodzić z **tej samej wersji** gry co grafiki (obecnie 2.1.21), inaczej maszyny rysują się jako prostokąty. Dump robi się przez `factorio --dump-data` (plik `script-output/data-raw-dump.json`).

Inny katalog danych: `--data <dir>` albo zmienna `FSIM_DATA`.

## CLI

```
cargo run --release -p fsim-cli -- gen 32x32 -e 30 --png out.png    # 30 losowych encji na 32x32, tekst na stdout
cargo run --release -p fsim-cli -- gen 40x24 -e 50 --seed 7 -o bp.txt --png out.png
cargo run --release -p fsim-cli -- gen --legacy chemical --png out.png  # stary generator szablonowy
cargo run --release -p fsim-cli -- render bp.txt -o out.png --grid
```

Render ma pod mapą pasek z naiwnym bilansem wejść i wyjść (`--no-summary` go wyłącza). Opis w `crates/fsim-cli/NOTES.md`.
