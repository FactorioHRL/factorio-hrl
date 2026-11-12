use std::collections::HashMap;

use fsim_blueprint::model::Design;
use fsim_calc::Flow;
use fsim_data::{GameDir, RawData};
use tiny_skia::{
    Color, FillRule, FilterQuality, IntRect, Paint, PathBuilder, Pixmap, PixmapPaint, Rect,
    Transform,
};

use crate::belts;
use crate::error::{Error, Result};
use crate::fluids::Connections;
use crate::icon::{product_icon, recipe_icon};
use crate::image::Image;
use crate::inserters::{self, Arm, CLAW_TIP_OVERSHOOT};
use crate::layout::{bounds, place, quarter_turns, Placed};
use crate::options::{Mode, Options};
use crate::palette::{color_for, darker, Rgb, BACKGROUND, GRID};
use crate::pipes;
use crate::sprite::{
    belt_layers, entity_layers, pipe_layers, splitter_layers, underground_layers, SpriteLayer,
};
use crate::summary::{self, Bar, Item};
use crate::text::{self, Weight};

const MAX_SIDE_PX: u32 = 16_384;
const MAX_SCALE: u32 = 512;
const NATIVE_PX_PER_TILE: f32 = 32.0;
const SHADOW_OPACITY: f32 = 0.55;

struct Frame {
    scale: f32,
    origin_x: f64,
    origin_y: f64,
}

impl Frame {
    fn x(&self, tile_x: f64) -> f32 {
        ((tile_x - self.origin_x) * f64::from(self.scale)) as f32
    }

    fn y(&self, tile_y: f64) -> f32 {
        ((tile_y - self.origin_y) * f64::from(self.scale)) as f32
    }
}

pub struct Renderer<'a> {
    data: &'a RawData,
    images: HashMap<String, Option<Pixmap>>,
    fallbacks: Vec<String>,
}

pub fn render(design: &Design, data: &RawData, options: &Options) -> Result<Image> {
    Renderer::new(data).render(design, options)
}

fn rank(kind: &str) -> u8 {
    match kind {
        "transport-belt" | "underground-belt" | "splitter" | "loader" | "loader-1x1" | "pipe"
        | "pipe-to-ground" | "heat-pipe" => 0,
        "inserter" => 2,
        "electric-pole" => 3,
        _ => 1,
    }
}

fn paint(rgb: Rgb, alpha: u8) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(rgb.0, rgb.1, rgb.2, alpha);
    paint.anti_alias = false;
    paint
}

fn fill_rect(canvas: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, rgb: Rgb, alpha: u8) {
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        canvas.fill_rect(rect, &paint(rgb, alpha), Transform::identity(), None);
    }
}

impl<'a> Renderer<'a> {
    pub fn new(data: &'a RawData) -> Self {
        Self {
            data,
            images: HashMap::new(),
            fallbacks: Vec::new(),
        }
    }

    /// Names of entities that fell back to a rectangle in the last `Sprites` mode render
    /// (no graphics, unsupported shape or missing file). Without repetitions.
    pub fn fallbacks(&self) -> &[String] {
        &self.fallbacks
    }

    pub fn render(&mut self, design: &Design, options: &Options) -> Result<Image> {
        if options.scale == 0 || options.scale > MAX_SCALE {
            return Err(Error::InvalidScale(options.scale));
        }
        let game_dir = match (options.mode, &options.game_dir) {
            (Mode::Sprites, None) => return Err(Error::GameDirRequired),
            (_, dir) => dir.as_ref(),
        };

        self.fallbacks.clear();
        let placed = place(design, self.data)?;
        let b = bounds(&placed)?;
        let frame = Frame {
            scale: options.scale as f32,
            origin_x: b.min_x - 1.0,
            origin_y: b.min_y - 1.0,
        };
        let side = |tiles: f64| (tiles * f64::from(options.scale)).ceil() as u64;
        let (width, height) = (side(b.max_x - b.min_x + 2.0), side(b.max_y - b.min_y + 2.0));
        if width > u64::from(MAX_SIDE_PX) || height > u64::from(MAX_SIDE_PX) {
            return Err(Error::ImageTooLarge {
                width,
                height,
                limit: MAX_SIDE_PX,
            });
        }
        let flows = if options.io_summary {
            fsim_calc::net_flows(design, self.data)?
        } else {
            Vec::new()
        };
        let bar = summary::layout(&flows, width as u32, options.scale);
        let map_height = height as u32;
        let (width, height) = match &bar {
            Some(bar) => (
                u64::from(bar.width.max(width as u32)),
                height + u64::from(bar.height),
            ),
            None => (width, height),
        };
        if width > u64::from(MAX_SIDE_PX) || height > u64::from(MAX_SIDE_PX) {
            return Err(Error::ImageTooLarge {
                width,
                height,
                limit: MAX_SIDE_PX,
            });
        }
        let mut canvas = Pixmap::new(width as u32, height as u32).ok_or(Error::ImageTooLarge {
            width,
            height,
            limit: MAX_SIDE_PX,
        })?;
        canvas.fill(Color::from_rgba8(
            BACKGROUND.0,
            BACKGROUND.1,
            BACKGROUND.2,
            255,
        ));

        if options.grid {
            draw_grid(&mut canvas, &frame, &b);
        }

        let mut order: Vec<usize> = (0..placed.len()).collect();
        order.sort_by_key(|&i| {
            let p = &placed[i];
            (rank(&p.kind), (p.cy * 2.0) as i64, (p.cx * 2.0) as i64, i)
        });

        let mut sprite_layers: Vec<Vec<SpriteLayer>> = vec![Vec::new(); placed.len()];
        let mut arms: Vec<Option<Arm>> = vec![None; placed.len()];
        if options.mode == Mode::Sprites {
            let belt_shapes = belts::shapes(&placed);
            let connections = Connections::new(design, &placed, self.data)?;
            let pipe_pictures = pipes::pictures(&placed, &connections);
            for &i in &order {
                let p = &placed[i];
                let Some(prototype) = self.data.get(&p.kind, &p.name) else {
                    continue;
                };
                sprite_layers[i] = match (belt_shapes[i], pipe_pictures[i]) {
                    (Some(shape), _) => {
                        belt_layers(prototype, &shape.variants(quarter_turns(p.direction)))
                    }
                    (None, Some(picture)) => pipe_layers(prototype, picture),
                    (None, None) if p.kind == inserters::INSERTER => {
                        arms[i] = inserters::arm(prototype, p.direction);
                        inserters::platform(prototype, p.direction)
                    }
                    (None, None) if p.kind == belts::SPLITTER => {
                        let facing = quarter_turns(p.direction);
                        let straight =
                            belts::BeltShape::Straight { start: false }.variants(facing)[0];
                        splitter_layers(prototype, p.direction, straight)
                    }
                    (None, None) if p.kind == belts::LOADER => {
                        underground_layers(prototype, p.direction, p.io, 0)
                    }
                    (None, None) if p.kind == belts::UNDERGROUND => {
                        let facing = quarter_turns(p.direction);
                        let straight =
                            belts::BeltShape::Straight { start: false }.variants(facing)[0];
                        underground_layers(prototype, p.direction, p.io, straight)
                    }
                    (None, None) => entity_layers(prototype, p.direction),
                };
            }
            if let Some(dir) = game_dir {
                for &i in &order {
                    for layer in sprite_layers[i].iter().filter(|l| l.shadow) {
                        self.draw_layer(&mut canvas, &frame, &placed[i], layer, dir);
                    }
                }
            }
        }

        for &i in &order {
            let mut drawn = false;
            if let (Mode::Sprites, Some(dir)) = (options.mode, game_dir) {
                for layer in sprite_layers[i].iter().filter(|l| !l.shadow) {
                    drawn |= self.draw_layer(&mut canvas, &frame, &placed[i], layer, dir);
                }
            }
            if let (true, Some(arm), Some(dir)) = (drawn, &arms[i], game_dir) {
                self.draw_arm(&mut canvas, &frame, &placed[i], arm, dir);
            }
            if !drawn {
                if options.mode == Mode::Sprites && !self.fallbacks.contains(&placed[i].name) {
                    self.fallbacks.push(placed[i].name.clone());
                }
                draw_schematic_box(&mut canvas, &frame, &placed[i]);
            }
        }

        if let (true, Some(dir)) = (options.recipe_icons, game_dir) {
            for &i in &order {
                if let Some(recipe) = &design.entities[i].recipe {
                    self.draw_recipe_icon(&mut canvas, &frame, &placed[i], &recipe.name, dir);
                }
            }
        }
        if options.arrows {
            for &i in &order {
                draw_arrow(&mut canvas, &frame, &placed[i]);
            }
        }
        if options.indices {
            for &i in &order {
                draw_index(&mut canvas, &frame, &placed[i], i);
            }
        }
        if let Some(bar) = &bar {
            self.draw_summary(&mut canvas, map_height, bar, &flows, game_dir);
        }
        Ok(Image::new(canvas))
    }

    /// Balance bar below the map: `Input:` and the list of inputs, `Output:` and the list of outputs, with
    /// an icon, name and rate per second in each row.
    fn draw_summary(
        &mut self,
        canvas: &mut Pixmap,
        top: u32,
        bar: &Bar,
        flows: &[Flow],
        dir: Option<&GameDir>,
    ) {
        const BAR_BACKGROUND: Rgb = (28, 28, 30);
        const BAR_RULE: Rgb = (90, 90, 96);
        const HEADER_TEXT: Rgb = (255, 255, 255);
        const NAME_TEXT: Rgb = (225, 225, 225);
        const INPUT_TEXT: Rgb = (255, 140, 120);
        const OUTPUT_TEXT: Rgb = (140, 230, 140);
        let top_f = top as f32;
        let (w, h) = (canvas.width() as f32, bar.height as f32);
        fill_rect(canvas, 0.0, top_f, w, h, BAR_BACKGROUND, 255);
        fill_rect(canvas, 0.0, top_f, w, 2.0, BAR_RULE, 255);
        // text is vertically centred relative to the icon
        let centred = |y: f32| y + (bar.icon - bar.font_px * 1.2) / 2.0;
        for item in &bar.items {
            match item {
                Item::Header { text, x, y } => {
                    text::draw(
                        canvas,
                        text,
                        *x,
                        top_f + *y,
                        bar.font_px * 1.15,
                        Weight::Bold,
                        HEADER_TEXT,
                    );
                }
                Item::Row {
                    flow,
                    icon_x,
                    y,
                    name,
                    name_x,
                    amount,
                    amount_x,
                } => {
                    let flow = &flows[*flow];
                    let y = top_f + *y;
                    let drawn = dir.is_some_and(|dir| {
                        self.draw_item_icon(canvas, &flow.name, *icon_x, y, bar.icon, dir)
                    });
                    if !drawn {
                        fill_rect(
                            canvas,
                            *icon_x,
                            y,
                            bar.icon,
                            bar.icon,
                            color_for(&flow.name),
                            255,
                        );
                    }
                    text::draw(
                        canvas,
                        name,
                        *name_x,
                        centred(y),
                        bar.font_px,
                        Weight::Regular,
                        NAME_TEXT,
                    );
                    let color = if flow.is_input() {
                        INPUT_TEXT
                    } else {
                        OUTPUT_TEXT
                    };
                    text::draw(
                        canvas,
                        amount,
                        *amount_x,
                        centred(y),
                        bar.font_px,
                        Weight::Bold,
                        color,
                    );
                }
            }
        }
    }

    fn draw_item_icon(
        &mut self,
        canvas: &mut Pixmap,
        name: &str,
        x: f32,
        y: f32,
        size: f32,
        dir: &GameDir,
    ) -> bool {
        let Some(icon) = product_icon(self.data, name) else {
            return false;
        };
        let Some(crop) = self.crop(dir, &icon.path, (0, 0, icon.size, icon.size)) else {
            return false;
        };
        let factor = size / icon.size as f32;
        blit(
            canvas,
            &crop,
            Transform::from_row(factor, 0.0, 0.0, factor, x, y),
            1.0,
        );
        true
    }

    fn load(&mut self, dir: &GameDir, path: &str) -> Option<&Pixmap> {
        let resolved = dir.resolve(path).ok()?;
        let key = resolved.to_string_lossy().into_owned();
        self.images
            .entry(key)
            .or_insert_with(|| Pixmap::load_png(&resolved).ok())
            .as_ref()
    }

    fn crop(
        &mut self,
        dir: &GameDir,
        filename: &str,
        area: (u32, u32, u32, u32),
    ) -> Option<Pixmap> {
        let rect = IntRect::from_xywh(area.0 as i32, area.1 as i32, area.2, area.3)?;
        self.load(dir, filename)?.clone_rect(rect)
    }

    fn draw_layer(
        &mut self,
        canvas: &mut Pixmap,
        frame: &Frame,
        placed: &Placed,
        layer: &SpriteLayer,
        dir: &GameDir,
    ) -> bool {
        let area = (layer.x, layer.y, layer.width, layer.height);
        let Some(crop) = self.crop(dir, &layer.filename, area) else {
            return false;
        };
        let factor = layer.scale as f32 * frame.scale / NATIVE_PX_PER_TILE;
        let cx = frame.x(placed.cx + layer.shift.0);
        let cy = frame.y(placed.cy + layer.shift.1);
        let left = cx - layer.width as f32 * factor / 2.0;
        let top = cy - layer.height as f32 * factor / 2.0;
        let opacity = if layer.shadow { SHADOW_OPACITY } else { 1.0 };
        blit(
            canvas,
            &crop,
            Transform::from_row(factor, 0.0, 0.0, factor, left, top),
            opacity,
        );
        true
    }

    fn draw_arm(
        &mut self,
        canvas: &mut Pixmap,
        frame: &Frame,
        placed: &Placed,
        arm: &Arm,
        dir: &GameDir,
    ) {
        let shoulder = Transform::from_translate(frame.x(placed.cx), frame.y(placed.cy))
            .pre_rotate(arm.angle_deg);
        let tile = f64::from(frame.scale);

        if let Some(layer) = &arm.base {
            let area = (layer.x, layer.y, layer.width, layer.height);
            if let Some(crop) = self.crop(dir, &layer.filename, area) {
                let factor = layer.scale as f32 * frame.scale / NATIVE_PX_PER_TILE;
                let (w, h) = (layer.width as f32 * factor, layer.height as f32 * factor);
                let transform = shoulder
                    .pre_translate(0.0, -h / 2.0)
                    .pre_rotate(180.0)
                    .pre_translate(-w / 2.0, -h / 2.0)
                    .pre_scale(factor, factor);
                blit(canvas, &crop, transform, 1.0);
            }
        }
        if let Some(layer) = &arm.hand {
            let area = (layer.x, layer.y, layer.width, layer.height);
            if let Some(crop) = self.crop(dir, &layer.filename, area) {
                let factor = layer.scale as f32 * frame.scale / NATIVE_PX_PER_TILE;
                let w = layer.width as f32 * factor;
                let tip = -((arm.reach + CLAW_TIP_OVERSHOOT) * tile) as f32;
                let transform = shoulder
                    .pre_translate(-w / 2.0, tip)
                    .pre_scale(factor, factor);
                blit(canvas, &crop, transform, 1.0);
            }
        }
    }

    fn draw_recipe_icon(
        &mut self,
        canvas: &mut Pixmap,
        frame: &Frame,
        placed: &Placed,
        recipe: &str,
        dir: &GameDir,
    ) {
        let Some(icon) = recipe_icon(self.data, recipe) else {
            return;
        };
        let spec = self
            .data
            .get(&placed.kind, &placed.name)
            .and_then(|p| p.get("icon_draw_specification"));
        let shift = spec
            .and_then(|s| s.get("shift"))
            .and_then(|s| Some((s.get(0)?.as_f64()?, s.get(1)?.as_f64()?)))
            .unwrap_or((0.0, 0.0));
        let scale = spec
            .and_then(|s| s.get("scale"))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(1.0) as f32;

        let Some(crop) = self.crop(dir, &icon.path, (0, 0, icon.size, icon.size)) else {
            return;
        };
        let factor = frame.scale * scale / icon.size as f32;
        let cx = frame.x(placed.cx + shift.0);
        let cy = frame.y(placed.cy + shift.1);
        let half = icon.size as f32 * factor / 2.0;
        blit(
            canvas,
            &crop,
            Transform::from_row(factor, 0.0, 0.0, factor, cx - half, cy - half),
            1.0,
        );
    }
}

fn blit(canvas: &mut Pixmap, crop: &Pixmap, transform: Transform, opacity: f32) {
    let paint = PixmapPaint {
        quality: FilterQuality::Bilinear,
        opacity,
        ..PixmapPaint::default()
    };
    canvas.draw_pixmap(0, 0, crop.as_ref(), &paint, transform, None);
}

fn draw_grid(canvas: &mut Pixmap, frame: &Frame, b: &crate::layout::Bounds) {
    let (w, h) = (canvas.width() as f32, canvas.height() as f32);
    let mut x = (b.min_x - 1.0).ceil();
    while x <= b.max_x + 1.0 {
        fill_rect(canvas, frame.x(x), 0.0, 1.0, h, GRID, 255);
        x += 1.0;
    }
    let mut y = (b.min_y - 1.0).ceil();
    while y <= b.max_y + 1.0 {
        fill_rect(canvas, 0.0, frame.y(y), w, 1.0, GRID, 255);
        y += 1.0;
    }
}

fn draw_schematic_box(canvas: &mut Pixmap, frame: &Frame, placed: &Placed) {
    let color = color_for(&placed.kind);
    let left = frame.x(placed.cx - placed.width / 2.0);
    let top = frame.y(placed.cy - placed.height / 2.0);
    let (w, h) = (
        placed.width as f32 * frame.scale,
        placed.height as f32 * frame.scale,
    );
    fill_rect(canvas, left, top, w, h, darker(color), 255);
    fill_rect(canvas, left + 1.0, top + 1.0, w - 2.0, h - 2.0, color, 255);
}

fn draw_arrow(canvas: &mut Pixmap, frame: &Frame, placed: &Placed) {
    let angle = (placed.direction.index() as f32) * std::f32::consts::TAU / 16.0;
    let (dx, dy) = (angle.sin(), -angle.cos());
    let (px, py) = (-dy, dx);
    let length = 0.3 * frame.scale * placed.width.min(placed.height) as f32;
    let (cx, cy) = (frame.x(placed.cx), frame.y(placed.cy));
    let mut builder = PathBuilder::new();
    builder.move_to(cx + dx * length, cy + dy * length);
    builder.line_to(
        cx - dx * length * 0.6 + px * length * 0.7,
        cy - dy * length * 0.6 + py * length * 0.7,
    );
    builder.line_to(
        cx - dx * length * 0.6 - px * length * 0.7,
        cy - dy * length * 0.6 - py * length * 0.7,
    );
    builder.close();
    if let Some(path) = builder.finish() {
        let mut p = paint((245, 245, 245), 255);
        p.anti_alias = true;
        canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_index(canvas: &mut Pixmap, frame: &Frame, placed: &Placed, index: usize) {
    let left = frame.x(placed.cx - placed.width / 2.0) + 2.0;
    let top = frame.y(placed.cy - placed.height / 2.0) + 1.0;
    let px = (frame.scale * 0.4).max(7.0);
    text::draw(
        canvas,
        &index.to_string(),
        left,
        top,
        px,
        Weight::Bold,
        (255, 255, 255),
    );
}
