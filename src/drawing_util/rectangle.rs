/// A rectangle drawing utility with optional styling.
/// Uses a builder pattern — only pay for what you use.
///
/// Example:
/// ```
/// Rectangle::new(0, 0, 200, 80)
///     .color(8, 5, 0)
///     .rounded(16.0)
///     .shadow(4, 4, 8.0, 0, 0, 0, 180)
///     .outline(1.0, 255, 255, 255, 80)
///     .draw(canvas, canvas_width, canvas_height);
/// ```
pub struct Rectangle {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    rgb: [u8; 3],
    a: u8,
    radius: f32,
    outline: Option<Outline>,
    shadow: Option<Shadow>,
}

struct Outline {
    width: f32,
    rgb: [u8; 3],
    a: u8,
}

struct Shadow {
    offset_x: i32,
    offset_y: i32,
    blur: f32,
    rgb: [u8; 3],
    a: u8,
}

impl Rectangle {
    #[inline]
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            rgb: [0, 0, 0],
            a: 255,
            radius: 0.0,
            outline: None,
            shadow: None,
        }
    }

    #[inline]
    pub fn color(mut self, rgb: [u8; 3]) -> Self {
        self.rgb = rgb;
        self
    }

    #[inline]
    pub fn alpha(mut self, a: u8) -> Self {
        self.a = a;
        self
    }

    #[inline]
    pub fn rounded(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    #[inline]
    pub fn outline(mut self, width: f32, rgb: [u8; 3], a: u8) -> Self {
        self.outline = Some(Outline { width, rgb, a });
        self
    }

    #[inline]
    pub fn shadow(mut self, offset_x: i32, offset_y: i32, blur: f32, rgb: [u8; 3], a: u8) -> Self {
        self.shadow = Some(Shadow { offset_x, offset_y, blur, rgb, a });
        self
    }

    /// Signed distance field for a rounded rectangle.
    /// Returns negative values inside, positive outside, zero on the boundary.
    /// `cx`, `cy` are the center of the rectangle in canvas space.
    #[inline]
    fn sdf(&self, px: f32, py: f32, cx: f32, cy: f32) -> f32 {
        let half_w = self.width as f32 / 2.0;
        let half_h = self.height as f32 / 2.0;
        let r = self.radius;

        // Distance from center, reduced by corner radius
        let qx = (px - cx).abs() - half_w + r;
        let qy = (py - cy).abs() - half_h + r;

        let len = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt();
        len + qx.min(0.0).max(qy.min(0.0)) - r
    }

    fn draw_shadow(&self, shadow: &Shadow, canvas: &mut [u8], canvas_width: u32, canvas_height: u32) {
        let stride = canvas_width as usize * 4;

        // Rectangle center in canvas space
        let cx = self.x as f32 + self.width as f32 / 2.0;
        let cy = self.y as f32 + self.height as f32 / 2.0;

        // Expand bounding box to cover blur spread + offset
        let spread = (shadow.blur * 3.0).ceil() as i32;
        let x_start = (self.x + shadow.offset_x - spread).max(0) as u32;
        let y_start = (self.y + shadow.offset_y - spread).max(0) as u32;
        let x_end = (self.x as i32 + self.width as i32 + shadow.offset_x + spread)
            .min(canvas_width as i32)
            .max(0) as u32;
        let y_end = (self.y as i32 + self.height as i32 + shadow.offset_y + spread)
            .min(canvas_height as i32)
            .max(0) as u32;

        // Precompute gaussian denominator
        let two_blur_sq = 2.0 * shadow.blur * shadow.blur;

        for py in y_start..y_end {
            for px in x_start..x_end {
                // Offset pixel back to get distance from the un-offset rectangle
                let dist = self.sdf(
                    px as f32 - shadow.offset_x as f32,
                    py as f32 - shadow.offset_y as f32,
                    cx,
                    cy,
                );

                // Only draw shadow outside or on the rectangle boundary
                if dist < 0.0 {
                    continue;
                }

                // Gaussian falloff
                let falloff = (-dist * dist / two_blur_sq).exp();
                let a = (shadow.a as f32 * falloff) as u8;
                if a == 0 {
                    continue;
                }

                let idx = py as usize * stride + px as usize * 4;

                // Premultiplied alpha blend over existing canvas
                let premul = |c: u8| (c as u32 * a as u32 / 255) as u8;
                let src_a = a as u32;
                let dst_a = canvas[idx + 3] as u32;
                let out_a = src_a + dst_a * (255 - src_a) / 255;

                if out_a == 0 {
                    continue;
                }

                canvas[idx]     = ((premul(shadow.rgb[2]) as u32 * 255 + canvas[idx]     as u32 * dst_a * (255 - src_a) / 255) / out_a) as u8;
                canvas[idx + 1] = ((premul(shadow.rgb[1]) as u32 * 255 + canvas[idx + 1] as u32 * dst_a * (255 - src_a) / 255) / out_a) as u8;
                canvas[idx + 2] = ((premul(shadow.rgb[0]) as u32 * 255 + canvas[idx + 2] as u32 * dst_a * (255 - src_a) / 255) / out_a) as u8;
                canvas[idx + 3] = out_a as u8;
            }
        }
    }

    pub fn draw(&self, canvas: &mut [u8], canvas_width: u32, canvas_height: u32) {
        // Shadow draws first so fill renders on top
        if let Some(shadow) = &self.shadow {
            self.draw_shadow(shadow, canvas, canvas_width, canvas_height);
        }

        let stride = canvas_width as usize * 4;
        let cx = self.x as f32 + self.width as f32 / 2.0;
        let cy = self.y as f32 + self.height as f32 / 2.0;

        let x_start = self.x.max(0) as u32;
        let y_start = self.y.max(0) as u32;
        let x_end = (self.x + self.width as i32).min(canvas_width as i32).max(0) as u32;
        let y_end = (self.y + self.height as i32).min(canvas_height as i32).max(0) as u32;

        for py in y_start..y_end {
            for px in x_start..x_end {
                let dist = self.sdf(px as f32, py as f32, cx, cy);

                // Outside rounded rect — skip
                if dist > 0.0 {
                    continue;
                }

                let idx = py as usize * stride + px as usize * 4;

                // Check if this pixel is on the outline (inset)
                let (r, g, b, a) = if let Some(ref ol) = self.outline {
                    // Pixel is on outline if it's within outline_width of the boundary
                    if dist >= -ol.width {
                        (ol.rgb[0], ol.rgb[1], ol.rgb[2], ol.a)
                    } else {
                        (self.rgb[0], self.rgb[1], self.rgb[2], self.a)
                    }
                } else {
                    (self.rgb[0], self.rgb[1], self.rgb[2], self.a)
                };

                // Smooth the boundary with anti-aliasing over 1px
                let edge_alpha = if dist > -1.0 {
                    (a as f32 * (-dist)) as u8
                } else {
                    a
                };

                if edge_alpha == 0 {
                    continue;
                }

                // Premultiplied alpha write
                let premul = |c: u8| (c as u32 * edge_alpha as u32 / 255) as u8;
                canvas[idx]     = premul(b);
                canvas[idx + 1] = premul(g);
                canvas[idx + 2] = premul(r);
                canvas[idx + 3] = edge_alpha;
            }
        }
    }
}
