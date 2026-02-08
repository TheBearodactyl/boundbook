use {
    clap::ValueEnum,
    lerp::Lerp,
    nalgebra::{Matrix2, Vector2},
};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum InterpolationMethod {
    /// Simple linear blending (fastest)
    Blend,
    /// Weighted blending with ease-in/ease-out
    Smooth,
    /// Cosine interpolation for smoother transitions
    Cosine,
    /// Cubic hermite spline interpolation
    Cubic,
    /// Perlin smoothstep (quintic hermite)
    Perlin,
    /// Exponential ease-in-out
    Exponential,
    /// Optical flow based (Lucas-Kanade sparse)
    OpticalFlowSparse,
    /// Motion-compensated blending (simplified Horn-Schunck)
    MotionCompensated,
    /// Catmull-Rom spline (requires 4 frames, falls back to cubic)
    CatmullRom,
}

pub struct FrameInterpolator {
    method: InterpolationMethod,
}

impl FrameInterpolator {
    pub const fn new(method: InterpolationMethod) -> Self {
        Self { method }
    }

    #[allow(clippy::arithmetic_side_effects)]
    pub fn ease_function(&self, t: f32, method: InterpolationMethod) -> f32 {
        match method {
            InterpolationMethod::Blend => t,
            InterpolationMethod::Smooth => t * t * (3.0 - 2.0 * t),
            InterpolationMethod::Cosine => (1.0 - f32::cos(t * std::f32::consts::PI)) / 2.0,
            InterpolationMethod::Cubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let f = 2.0 * t - 2.0;
                    1.0 + f * f * f / 2.0
                }
            }
            InterpolationMethod::Perlin => t * t * t * (t * (t * 6.0 - 15.0) + 10.0),
            InterpolationMethod::Exponential => {
                if t < 0.5 {
                    0.5 * f32::powf(2.0, 20.0 * t - 10.0)
                } else {
                    1.0 - 0.5 * f32::powf(2.0, -20.0 * t + 10.0)
                }
            }
            InterpolationMethod::OpticalFlowSparse
            | InterpolationMethod::MotionCompensated
            | InterpolationMethod::CatmullRom => t,
        }
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn compute_gradients(
        &self,
        data: &[u8],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
    ) -> (f32, f32) {
        let idx = (y * width + x) * 4;

        if x == 0 || y == 0 || x >= width - 1 || y >= height - 1 {
            return (0.0, 0.0);
        }

        let to_gray = |idx: usize| -> f32 {
            let r = data[idx] as f32;
            let g = data[idx + 1] as f32;
            let b = data[idx + 2] as f32;
            0.299 * r + 0.587 * g + 0.114 * b
        };

        let left = to_gray(idx - 4);
        let right = to_gray(idx + 4);
        let top = to_gray(idx - width * 4);
        let bottom = to_gray(idx + width * 4);

        let gx = (right - left) / 2.0;
        let gy = (bottom - top) / 2.0;

        (gx, gy)
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn compute_optical_flow_sparse(
        &self,
        frame1: &[u8],
        frame2: &[u8],
        width: usize,
        height: usize,
    ) -> Vec<(usize, usize, f32, f32)> {
        let mut flow_vectors = Vec::new();
        let window_size = 5;
        let stride = 16;

        for y in (window_size..height - window_size).step_by(stride) {
            for x in (window_size..width - window_size).step_by(stride) {
                let mut a_mat = Matrix2::zeros();
                let mut b_vec = Vector2::zeros();

                for dy in -(window_size as i32)..=(window_size as i32) {
                    for dx in -(window_size as i32)..=(window_size as i32) {
                        let px = (x as i32 + dx) as usize;
                        let py = (y as i32 + dy) as usize;

                        let (gx, gy) = self.compute_gradients(frame1, width, height, px, py);

                        let idx1 = (py * width + px) * 4;
                        let idx2 = (py * width + px) * 4;

                        let i1 = frame1[idx1] as f32;
                        let i2 = frame2[idx2] as f32;
                        let it = i2 - i1;

                        a_mat[(0, 0)] += gx * gx;
                        a_mat[(0, 1)] += gx * gy;
                        a_mat[(1, 0)] += gx * gy;
                        a_mat[(1, 1)] += gy * gy;

                        b_vec[0] -= gx * it;
                        b_vec[1] -= gy * it;
                    }
                }

                if let Some(inv) = a_mat.try_inverse() {
                    let flow = inv * b_vec;
                    let vx: f32 = flow[0];
                    let vy: f32 = flow[1];

                    if vx.abs() < 10.0 && vy.abs() < 10.0 {
                        flow_vectors.push((x, y, vx, vy));
                    }
                }
            }
        }

        flow_vectors
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn interpolate_with_optical_flow(
        &self,
        frame1: &[u8],
        frame2: &[u8],
        width: usize,
        height: usize,
        t: f32,
    ) -> Vec<u8> {
        let flow = self.compute_optical_flow_sparse(frame1, frame2, width, height);
        let mut result = frame1.to_vec();

        for (fx, fy, vx, vy) in flow {
            let radius = 8;

            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let x = (fx as i32 + dx).clamp(0, width as i32 - 1) as usize;
                    let y = (fy as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    let src_x = (x as f32 - vx * t).clamp(0.0, width as f32 - 1.0);
                    let src_y = (y as f32 - vy * t).clamp(0.0, height as f32 - 1.0);
                    let x0 = src_x.floor() as usize;
                    let x1 = (x0 + 1).min(width - 1);
                    let y0 = src_y.floor() as usize;
                    let y1 = (y0 + 1).min(height - 1);
                    let wx = src_x - x0 as f32;
                    let wy = src_y - y0 as f32;
                    let idx = (y * width + x) * 4;

                    for c in 0..4 {
                        let v00 = frame2[(y0 * width + x0) * 4 + c] as f32;
                        let v01 = frame2[(y0 * width + x1) * 4 + c] as f32;
                        let v10 = frame2[(y1 * width + x0) * 4 + c] as f32;
                        let v11 = frame2[(y1 * width + x1) * 4 + c] as f32;

                        let v0 = v00.lerp(v01, wx);
                        let v1 = v10.lerp(v11, wx);
                        let v = v0.lerp(v1, wy);
                        let orig = frame1[idx + c] as f32;
                        result[idx + c] = orig.lerp(v, t) as u8;
                    }
                }
            }
        }

        result
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn interpolate_motion_compensated(
        &self,
        frame1: &[u8],
        frame2: &[u8],
        width: usize,
        height: usize,
        t: f32,
    ) -> Vec<u8> {
        let mut result = vec![0u8; frame1.len()];

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let idx = (y * width + x) * 4;

                for c in 0..4 {
                    let (gx, _) = self.compute_gradients(frame1, width, height, x, y);
                    let i1 = frame1[idx + c] as f32;
                    let i2 = frame2[idx + c] as f32;
                    let it = i2 - i1;
                    let motion = if gx.abs() > 0.1 { -it / gx * t } else { 0.0 };
                    let src_x = (x as f32 + motion).clamp(0.0, width as f32 - 1.0);
                    let v1 = i1;
                    let v2 = self.bilinear_sample(frame2, width, height, src_x, y as f32, c);

                    result[idx + c] = v1.lerp(v2, t) as u8;
                }
            }
        }

        result
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn bilinear_sample(
        &self,
        data: &[u8],
        width: usize,
        height: usize,
        x: f32,
        y: f32,
        channel: usize,
    ) -> f32 {
        let x0 = x.floor() as usize;
        let x1 = (x0 + 1).min(width - 1);
        let y0 = y.floor() as usize;
        let y1 = (y0 + 1).min(height - 1);

        let wx = x - x0 as f32;
        let wy = y - y0 as f32;

        let v00 = data[(y0 * width + x0) * 4 + channel] as f32;
        let v01 = data[(y0 * width + x1) * 4 + channel] as f32;
        let v10 = data[(y1 * width + x0) * 4 + channel] as f32;
        let v11 = data[(y1 * width + x1) * 4 + channel] as f32;

        let v0 = v00.lerp(v01, wx);
        let v1 = v10.lerp(v11, wx);
        v0.lerp(v1, wy)
    }

    /// Interpolates between two animation frames
    ///
    /// # Panics
    ///
    /// Panics if frame1 and frame2 have different lengths
    pub fn interpolate_frames(
        &self,
        frame1: &[u8],
        frame2: &[u8],
        width: u32,
        height: u32,
        t: f32,
    ) -> Vec<u8> {
        assert_eq!(frame1.len(), frame2.len());

        match self.method {
            InterpolationMethod::OpticalFlowSparse => self.interpolate_with_optical_flow(
                frame1,
                frame2,
                width as usize,
                height as usize,
                t,
            ),
            InterpolationMethod::MotionCompensated => self.interpolate_motion_compensated(
                frame1,
                frame2,
                width as usize,
                height as usize,
                t,
            ),
            _ => {
                let adjusted_t = self.ease_function(t, self.method);
                frame1
                    .iter()
                    .zip(frame2.iter())
                    .map(|(a, b)| {
                        let a_f = *a as f32;
                        let b_f = *b as f32;
                        a_f.lerp(b_f, adjusted_t) as u8
                    })
                    .collect()
            }
        }
    }
}
