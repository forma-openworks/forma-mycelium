//! Иконка Forma Mycelium, нарисованная кодом: изометрический блок мицелия в духе Майнкрафта.
//! Верх — в фиолетовом градиенте FORMA, по нему светлые нити с узелками (как связи на карте
//! мода), по бокам «нависающий» пиксельный край, а сверху волчьи ушки, как у Holotype.
//! Файл ни от чего не зависит — его же подключает build.rs, чтобы вшить иконку в .exe.

type V = [f32; 2];

fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot(a: V, b: V) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}
fn scale(a: V, k: f32) -> V {
    [a[0] * k, a[1] * k]
}

/// Расстояние со знаком до треугольника (отрицательное — внутри).
fn sd_triangle(p: V, a: V, b: V, c: V) -> f32 {
    let (e0, e1, e2) = (sub(b, a), sub(c, b), sub(a, c));
    let (v0, v1, v2) = (sub(p, a), sub(p, b), sub(p, c));
    let proj = |v: V, e: V| sub(v, scale(e, (dot(v, e) / dot(e, e)).clamp(0.0, 1.0)));
    let (q0, q1, q2) = (proj(v0, e0), proj(v1, e1), proj(v2, e2));
    let s = (e0[0] * e2[1] - e0[1] * e2[0]).signum();
    let cross = |v: V, e: V| s * (v[0] * e[1] - v[1] * e[0]);
    let d = dot(q0, q0).min(dot(q1, q1)).min(dot(q2, q2));
    let inside = cross(v0, e0).min(cross(v1, e1)).min(cross(v2, e2));
    -d.sqrt() * inside.signum()
}

/// Расстояние до отрезка с толщиной.
fn sd_capsule(p: V, a: V, b: V, r: f32) -> f32 {
    let pa = sub(p, a);
    let ba = sub(b, a);
    let h = (dot(pa, ba) / dot(ba, ba)).clamp(0.0, 1.0);
    let d = sub(pa, scale(ba, h));
    dot(d, d).sqrt() - r
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn shade(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// Псевдослучайное число 0..1 для клетки пиксельной текстуры.
fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA77);
    x ^= x >> 13;
    (x & 0xffff) as f32 / 65535.0
}

// Вершины куба: верхняя грань — ромб L–T–R–C, боковые грани опускаются на высоту H.
const T: V = [0.5, 0.31];
const L: V = [0.11, 0.5];
const R: V = [0.89, 0.5];
const C: V = [0.5, 0.69];
const H: f32 = 0.29;
/// Сколько «пикселей» по стороне грани, как у текстуры блока.
const PIXELS: f32 = 8.0;

/// Волчье ушко на заднем левом ребре верхней грани; правое — зеркальное.
const EAR: [V; 3] = [[0.18, 0.466], [0.391, 0.363], [0.155, 0.06]];
const EAR_INNER: [V; 3] = [[0.227, 0.443], [0.352, 0.382], [0.19, 0.17]];

fn mirror(t: [V; 3]) -> [V; 3] {
    t.map(|p| [1.0 - p[0], p[1]])
}

/// Нити мицелия на верхней грани (в координатах картинки): узел-центр, ветви с узелками
/// и короткие отростки, чуть несимметрично, как живая грибница и связи на карте мода.
const NODES: [V; 9] = [
    [0.5, 0.5],
    [0.29, 0.465],
    [0.585, 0.385],
    [0.735, 0.515],
    [0.43, 0.615],
    [0.2, 0.505],
    [0.665, 0.36],
    [0.8, 0.475],
    [0.36, 0.585],
];
const THREADS: [(usize, usize); 8] = [(0, 1), (0, 2), (0, 3), (0, 4), (1, 5), (2, 6), (3, 7), (2, 3)];
/// Радиус узла: центральный крупнее, на концах отростков — точки.
const NODE_R: [f32; 9] = [0.03, 0.021, 0.021, 0.021, 0.021, 0.012, 0.012, 0.012, 0.0];

/// Координаты точки на грани-параллелограмме: начало `o`, рёбра `e1` и `e2`.
fn face_uv(p: V, o: V, e1: V, e2: V) -> V {
    let d = sub(p, o);
    let det = e1[0] * e2[1] - e1[1] * e2[0];
    [(d[0] * e2[1] - d[1] * e2[0]) / det, (e1[0] * d[1] - e1[1] * d[0]) / det]
}

fn inside(uv: V) -> bool {
    (0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1])
}

/// Яркость пиксельной клетки грани: лёгкий «шум» как у текстур Майнкрафта.
fn speckle(uv: V, face: u32, amount: f32) -> f32 {
    let cx = (uv[0] * PIXELS).floor().clamp(0.0, PIXELS - 1.0) as u32;
    let cy = (uv[1] * PIXELS).floor().clamp(0.0, PIXELS - 1.0) as u32;
    1.0 + (hash(face * 1000 + cy * 16 + cx) - 0.5) * 2.0 * amount
}

/// Цвет и непрозрачность в точке (u, v) ∈ [0, 1]². `noise` — сила пиксельного шума.
fn sample(p: V, noise: f32) -> ([f32; 3], f32) {
    let top_light = [178.0, 150.0, 255.0];
    let top_deep = [124.0, 92.0, 238.0];

    // Верхняя грань: u — вдоль заднего левого ребра, v — вдоль переднего левого.
    let top = face_uv(p, L, sub(T, L), sub(C, L));
    if inside(top) {
        let t = (0.65 * top[1] + 0.35 * (1.0 - top[0])).clamp(0.0, 1.0);
        let mut col = shade(mix(top_light, top_deep, t), speckle(top, 1, noise));
        let mut d = f32::MAX;
        for (a, b) in THREADS {
            d = d.min(sd_capsule(p, NODES[a], NODES[b], 0.0085));
        }
        for (n, r) in NODES.iter().zip(NODE_R) {
            let q = sub(p, *n);
            d = d.min(dot(q, q).sqrt() - r);
        }
        if d < 0.0 {
            col = mix(col, [242.0, 234.0, 255.0], 0.92);
        }
        return (col, 1.0);
    }

    // Боковые грани с «нависающим» краем верхней текстуры.
    let side = |uv: V, base: [f32; 3], overhang: [f32; 3], face: u32| {
        let column = (uv[0] * PIXELS).floor().clamp(0.0, PIXELS - 1.0) as u32;
        let depth = 0.14 + 0.16 * hash(face * 77 + column);
        let row_depth = (depth * PIXELS).ceil() / PIXELS;
        let c = if uv[1] < row_depth { overhang } else { base };
        shade(c, speckle(uv, face, noise))
    };
    let left = face_uv(p, L, sub(C, L), [0.0, H]);
    if inside(left) {
        return (side(left, [92.0, 66.0, 196.0], [146.0, 114.0, 246.0], 2), 1.0);
    }
    let right = face_uv(p, C, sub(R, C), [0.0, H]);
    if inside(right) {
        return (side(right, [66.0, 46.0, 160.0], [118.0, 88.0, 222.0], 3), 1.0);
    }

    // Ушки стоят за верхней гранью, поэтому проверяются последними.
    let ear = |t: [V; 3]| sd_triangle(p, t[0], t[1], t[2]) - 0.016;
    if ear(EAR) < 0.0 || ear(mirror(EAR)) < 0.0 {
        let inner = |t: [V; 3]| sd_triangle(p, t[0], t[1], t[2]) - 0.01;
        let col = if inner(EAR_INNER) < 0.0 || inner(mirror(EAR_INNER)) < 0.0 {
            [255.0, 198.0, 234.0]
        } else {
            mix(top_light, top_deep, (p[1] * 2.2).clamp(0.0, 1.0))
        };
        return (col, 1.0);
    }
    ([0.0; 3], 0.0)
}

/// Картинка иконки размером `size`×`size` в RGBA (без премультипликации),
/// со сглаживанием суперсэмплингом 4×4.
pub fn rgba(size: u32) -> Vec<u8> {
    const SS: u32 = 4;
    // На маленьких размерах шум превращается в грязь — там грани ровные.
    let noise = if size >= 40 { 0.07 } else { 0.0 };
    let n = size as usize;
    let mut out = vec![0u8; n * n * 4];
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0.0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let u = (x as f32 + (sx as f32 + 0.5) / SS as f32) / size as f32;
                    let v = (y as f32 + (sy as f32 + 0.5) / SS as f32) / size as f32;
                    let (c, a) = sample([u, v], noise);
                    acc[0] += c[0] * a;
                    acc[1] += c[1] * a;
                    acc[2] += c[2] * a;
                    acc[3] += a;
                }
            }
            if acc[3] <= 0.0 {
                continue;
            }
            let i = (y as usize * n + x as usize) * 4;
            out[i] = (acc[0] / acc[3]).round().clamp(0.0, 255.0) as u8;
            out[i + 1] = (acc[1] / acc[3]).round().clamp(0.0, 255.0) as u8;
            out[i + 2] = (acc[2] / acc[3]).round().clamp(0.0, 255.0) as u8;
            out[i + 3] = (acc[3] / (SS * SS) as f32 * 255.0).round() as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn icon_shape() {
        let n = 64;
        let px = super::rgba(n);
        let alpha = |x: u32, y: u32| px[((y * n + x) * 4 + 3) as usize];
        assert_eq!(alpha(0, 0), 0, "углы прозрачные");
        assert_eq!(alpha(63, 63), 0, "углы прозрачные");
        assert_eq!(alpha(32, 9), 0, "между ушками прозрачно");
        assert_eq!(alpha(32, 40), 255, "куб непрозрачный");
        assert!(alpha(10, 6) > 0 && alpha(53, 6) > 0, "кончики ушек на месте");
        // ICON_PREVIEW=папка — сохранить картинки, чтобы посмотреть глазами.
        if let Some(dir) = std::env::var_os("ICON_PREVIEW") {
            for size in [16u32, 32, 48, 256] {
                let path = std::path::Path::new(&dir).join(format!("icon-{size}.png"));
                save_png(&path, size, &super::rgba(size));
            }
        }
    }

    /// Простейшая запись PNG без сжатия (чтобы не тянуть зависимость ради теста).
    fn save_png(path: &std::path::Path, size: u32, rgba: &[u8]) {
        fn crc(data: &[u8]) -> u32 {
            let mut c = 0xffff_ffffu32;
            for &b in data {
                c ^= b as u32;
                for _ in 0..8 {
                    c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
                }
            }
            !c
        }
        fn adler(data: &[u8]) -> u32 {
            let (mut a, mut b) = (1u32, 0u32);
            for &d in data {
                a = (a + d as u32) % 65521;
                b = (b + a) % 65521;
            }
            (b << 16) | a
        }
        let (w, h) = (size, size);
        let mut raw = Vec::new();
        for row in rgba.chunks(w as usize * 4) {
            raw.push(0u8);
            raw.extend_from_slice(row);
        }
        let mut z = vec![0x78, 0x01];
        for (i, chunk) in raw.chunks(65535).enumerate() {
            let last = (i + 1) * 65535 >= raw.len();
            z.push(last as u8);
            z.extend_from_slice(&(chunk.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!(chunk.len() as u16)).to_le_bytes());
            z.extend_from_slice(chunk);
        }
        z.extend_from_slice(&adler(&raw).to_be_bytes());
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        let mut chunk = |kind: &[u8], data: &[u8]| {
            png.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let mut body = kind.to_vec();
            body.extend_from_slice(data);
            png.extend_from_slice(&body);
            png.extend_from_slice(&crc(&body).to_be_bytes());
        };
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(b"IHDR", &ihdr);
        chunk(b"IDAT", &z);
        chunk(b"IEND", &[]);
        std::fs::write(path, png).unwrap();
    }
}
