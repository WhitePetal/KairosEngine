//! The loupe's layout, text and tiny bitmap font, apart from any window system.
//!
//! The X11 backend owns the window and the pixels; this module owns the
//! geometry, the text and the bitmap font. macOS uses the same point geometry
//! (for_points) but draws native text. Keeping it platform-free means the edge
//! flipping, the layout and the glyphs can be tested without an X server, and
//! the backend only has to decode a 15x15 block into RGB and copy the painted
//! pixels into its image.

pub(super) const GRID: usize = 15;
pub(super) const HALF: usize = GRID / 2;

const PANEL: [u8; 3] = [32, 32, 32];
const BORDER: [u8; 3] = [64, 64, 64];
const ACCENT: [u8; 3] = [64, 150, 224];
const WHITE: [u8; 3] = [255, 255, 255];
const TEXT: [u8; 3] = [232, 232, 232];
const DIM: [u8; 3] = [148, 148, 148];

/// The panel: a magnified 15x15 grid plus the hex, RGB and a colour chip.
pub(super) struct Loupe {
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) cell: u16,
    pub(super) margin: u16,
    pub(super) text_top: u16,
    pub(super) line: u16,
    font: u16,
    pub(super) chip: u16,
    scale: u16,
}

impl Loupe {
    /// Lay the panel out for a display of "dpi" dots per inch.
    ///
    /// The grid keeps the Windows loupe's 8 px cells; the text is one step
    /// larger than the DPI scale so it stays readable at 96 dpi.
    pub(super) fn for_dpi(dpi: f32) -> Self {
        let scale = scale_from_dpi(dpi);
        let cell = 8 * scale;
        let grid = GRID as u16 * cell;
        let margin = 8 * scale;
        let font = (scale + 1).min(4);
        let advance = 6 * font;
        let hex_width = (7 * advance).saturating_sub(font);
        let numbers_width = (11 * advance).saturating_sub(font);
        let chip = 14 * scale;
        let gap = 3 * scale;
        let text_width = (hex_width + gap + chip).max(numbers_width);
        let content = grid.max(text_width);
        let width = margin * 2 + content;
        let text_top = margin + grid + 3 * scale;
        let line = 7 * font + 3 * scale;
        let height = text_top + line * 2 + margin;

        Self {
            width,
            height,
            cell,
            margin,
            text_top,
            line,
            font,
            chip,
            scale,
        }
    }

    /// The Windows loupe's point geometry: 8 pt cells, a 136x165 pt panel and
    /// 11 pt text. macOS draws with native text but keeps the same panel.
    #[cfg(any(target_os = "macos", test))]
    pub(super) fn for_points() -> Self {
        let scale = 1;
        let cell = 8;
        let margin = 8;
        let grid = GRID as u16 * cell;
        let text_top = margin + grid + 3 * scale;
        let width = margin * 2 + grid;
        let height = text_top + 14 + 12 + margin;

        Self {
            width,
            height,
            cell,
            margin,
            text_top,
            line: 14,
            font: 1,
            chip: 14,
            scale,
        }
    }

    /// Top-left corner for the panel while the pointer is at "cursor" on a
    /// screen of "screen" physical pixels.
    ///
    /// The panel prefers the down-right of the pointer and flips to the other
    /// side when it would leave the screen, so it never covers the sampled
    /// pixel.
    pub(super) fn place(&self, cursor: (i16, i16), screen: (u16, u16)) -> (i16, i16) {
        let gap = 18 * i32::from(self.scale);
        let (cursor_x, cursor_y) = (i32::from(cursor.0), i32::from(cursor.1));
        let (width, height) = (i32::from(self.width), i32::from(self.height));
        let (screen_width, screen_height) = (i32::from(screen.0), i32::from(screen.1));

        let mut left = cursor_x + gap;
        let mut top = cursor_y + gap;
        if left + width > screen_width {
            left = cursor_x - gap - width;
        }
        if top + height > screen_height {
            top = cursor_y - gap - height;
        }
        let max_left = (screen_width - width).max(0);
        let max_top = (screen_height - height).max(0);

        (left.clamp(0, max_left) as i16, top.clamp(0, max_top) as i16)
    }

    /// Paint one frame. "block" is the 15x15 sampled block as 0x00RRGGBB.
    pub(super) fn paint<F>(&self, block: &[u32; GRID * GRID], srgb: [u8; 4], mut draw: F)
    where
        F: FnMut(u16, u16, [u8; 3]),
    {
        draw_rect(&mut draw, 0, 0, self.width, self.height, PANEL);
        draw_frame(&mut draw, 0, 0, self.width, self.height, BORDER);

        for row in 0..GRID as u16 {
            for column in 0..GRID as u16 {
                let pixel = block[usize::from(row) * GRID + usize::from(column)];
                let rgb = [(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8];
                draw_rect(
                    &mut draw,
                    self.margin + column * self.cell,
                    self.margin + row * self.cell,
                    self.cell,
                    self.cell,
                    rgb,
                );
            }
        }

        // The sampled pixel: an accent outline plus a white inner line so the
        // marker reads on both bright and dark pixels.
        let center = self.margin + HALF as u16 * self.cell;
        draw_frame(&mut draw, center, center, self.cell, self.cell, ACCENT);
        draw_frame(
            &mut draw,
            center + 1,
            center + 1,
            self.cell - 2,
            self.cell - 2,
            WHITE,
        );

        let hex = format!("#{:02X}{:02X}{:02X}", srgb[0], srgb[1], srgb[2]);
        draw_text(&mut draw, self.margin, self.text_top, &hex, TEXT, self.font);
        let numbers = format!("{} {} {}", srgb[0], srgb[1], srgb[2]);
        draw_text(
            &mut draw,
            self.margin,
            self.text_top + self.line,
            &numbers,
            DIM,
            self.font,
        );

        let chip_x = self.width - self.margin - self.chip;
        draw_rect(
            &mut draw,
            chip_x,
            self.text_top,
            self.chip,
            self.chip,
            [srgb[0], srgb[1], srgb[2]],
        );
        draw_frame(
            &mut draw,
            chip_x,
            self.text_top,
            self.chip,
            self.chip,
            BORDER,
        );
    }
}

/// DPI to an integer scale factor, clamped to what the layout can hold.
fn scale_from_dpi(dpi: f32) -> u16 {
    if !dpi.is_finite() || dpi <= 0.0 {
        return 1;
    }
    ((dpi / 96.0).round() as i32).clamp(1, 3) as u16
}

/// The Xft.dpi resource from an xrdb-style property, if present.
pub(super) fn parse_xft_dpi(resources: &[u8]) -> Option<f32> {
    let text = std::str::from_utf8(resources).ok()?;
    text.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if key.trim() != "Xft.dpi" {
            return None;
        }
        value.trim().parse::<f32>().ok()
    })
}

fn draw_rect<F>(draw: &mut F, x: u16, y: u16, width: u16, height: u16, color: [u8; 3])
where
    F: FnMut(u16, u16, [u8; 3]),
{
    for row in y..y.saturating_add(height) {
        for column in x..x.saturating_add(width) {
            draw(column, row, color);
        }
    }
}

fn draw_frame<F>(draw: &mut F, x: u16, y: u16, width: u16, height: u16, color: [u8; 3])
where
    F: FnMut(u16, u16, [u8; 3]),
{
    if width == 0 || height == 0 {
        return;
    }
    draw_rect(draw, x, y, width, 1, color);
    draw_rect(draw, x, y + height - 1, width, 1, color);
    draw_rect(draw, x, y, 1, height, color);
    draw_rect(draw, x + width - 1, y, 1, height, color);
}

fn draw_text<F>(draw: &mut F, x: u16, y: u16, text: &str, color: [u8; 3], font: u16)
where
    F: FnMut(u16, u16, [u8; 3]),
{
    let mut pen = x;
    for character in text.chars() {
        let glyph = glyph(character);
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..5u16 {
                if bits & (1 << (4 - column)) != 0 {
                    draw_rect(
                        draw,
                        pen + column * font,
                        y + row as u16 * font,
                        font,
                        font,
                        color,
                    );
                }
            }
        }
        pen += 6 * font;
    }
}

/// One 5x7 glyph, row by row, most significant bit leftmost.
fn glyph(character: char) -> [u8; 7] {
    match character {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        '#' => [
            0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010,
        ],
        _ => [0; 7],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_rounds_to_whole_steps_and_clamps() {
        assert_eq!(scale_from_dpi(96.0), 1);
        assert_eq!(scale_from_dpi(120.0), 1);
        assert_eq!(scale_from_dpi(144.0), 2);
        assert_eq!(scale_from_dpi(192.0), 2);
        assert_eq!(scale_from_dpi(288.0), 3);
        assert_eq!(scale_from_dpi(600.0), 3);
        assert_eq!(scale_from_dpi(0.0), 1);
        assert_eq!(scale_from_dpi(f32::NAN), 1);
    }

    #[test]
    fn xft_dpi_is_read_from_the_resource_manager() {
        let resources = b"*background:\t#000000\nXft.dpi:\t192\nXft.antialias:\t1\n";
        assert_eq!(parse_xft_dpi(resources), Some(192.0));
        assert_eq!(parse_xft_dpi(b"Xft.hinting: 1\n"), None);
        assert_eq!(parse_xft_dpi(b"Xft.dpi: not-a-number\n"), None);
        assert_eq!(parse_xft_dpi(b"\xff"), None);
    }

    #[test]
    fn the_native_panel_matches_the_windows_geometry() {
        let loupe = Loupe::for_points();
        assert_eq!(loupe.width, 136);
        assert_eq!(loupe.height, 165);
        assert_eq!(loupe.cell, 8);
        assert_eq!(loupe.margin, 8);
        assert_eq!(loupe.text_top, 131);
        assert_eq!(loupe.chip, 14);
    }

    #[test]
    fn the_panel_fits_its_own_text_and_grid() {
        for dpi in [96.0, 144.0, 192.0, 240.0] {
            let loupe = Loupe::for_dpi(dpi);
            let grid = GRID as u16 * loupe.cell;
            assert!(loupe.width >= grid + loupe.margin * 2, "dpi {dpi}");
            assert!(loupe.height >= loupe.text_top + loupe.line * 2, "dpi {dpi}");
            assert!(loupe.width >= loupe.margin * 2 + loupe.chip, "dpi {dpi}");
        }
    }

    #[test]
    fn the_panel_flips_away_from_the_screen_edges() {
        let loupe = Loupe::for_dpi(96.0);
        let screen = (1920, 1080);

        let (x, y) = loupe.place((100, 100), screen);
        assert!(x > 100 && y > 100);
        assert!(x + loupe.width as i16 <= screen.0 as i16);
        assert!(y + loupe.height as i16 <= screen.1 as i16);

        let (x, y) = loupe.place((1910, 1070), screen);
        assert!(x < 1910 && y < 1070);
        assert!(x >= 0 && y >= 0);

        let (x, y) = loupe.place((0, 0), screen);
        assert!(x > 0 && y > 0);
        assert!(x + loupe.width as i16 <= screen.0 as i16);
        assert!(y + loupe.height as i16 <= screen.1 as i16);
    }

    #[test]
    fn the_panel_paints_the_centre_cell_and_the_chip() {
        let loupe = Loupe::for_dpi(96.0);
        let mut block = [0u32; GRID * GRID];
        block[HALF * GRID + HALF] = 0x12_34_56;

        let mut pixels = vec![[0u8; 3]; usize::from(loupe.width) * usize::from(loupe.height)];
        loupe.paint(&block, [0xAB, 0xCD, 0xEF, 255], |x, y, rgb| {
            let index = usize::from(y) * usize::from(loupe.width) + usize::from(x);
            pixels[index] = rgb;
        });

        let center_x = loupe.margin + HALF as u16 * loupe.cell;
        let center_y = center_x;
        // The white inset is one pixel in from the accent outline.
        assert_eq!(
            pixels
                [usize::from(center_y + 1) * usize::from(loupe.width) + usize::from(center_x + 1)],
            WHITE
        );
        assert_eq!(
            pixels[usize::from(center_y) * usize::from(loupe.width) + usize::from(center_x)],
            ACCENT
        );

        let chip_x = loupe.width - loupe.margin - loupe.chip;
        let chip = pixels
            [usize::from(loupe.text_top + 1) * usize::from(loupe.width) + usize::from(chip_x + 1)];
        assert_eq!(chip, [0xAB, 0xCD, 0xEF]);
    }

    #[test]
    fn every_character_the_loupe_uses_has_a_glyph() {
        for character in "#0123456789ABCDEF".chars() {
            assert!(
                glyph(character).iter().any(|row| *row != 0),
                "no glyph for {character:?}"
            );
        }
        assert_eq!(glyph(' '), [0; 7]);
    }
}
