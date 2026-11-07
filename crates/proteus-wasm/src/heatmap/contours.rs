// contours.rs stitches cell boundaries and groups shells with their holes

use super::Point2D;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
pub(super) struct Vertex {
    pub id: usize,
    pub point: Point2D,
}

#[derive(Default)]
pub(super) struct Boundaries {
    vertices: HashMap<usize, usize>,
    points: Vec<Point2D>,
    edges: HashSet<(usize, usize)>,
}

impl Boundaries {
    fn vertex(&mut self, vertex: Vertex) -> usize {
        *self.vertices.entry(vertex.id).or_insert_with(|| {
            let index = self.points.len();
            self.points.push(vertex.point);
            index
        })
    }

    /** cancel shared edges using grid identities rather than rounded coordinates */
    pub fn edge(&mut self, from: Vertex, to: Vertex) {
        if from.id == to.id {
            return;
        }
        let from = self.vertex(from);
        let to = self.vertex(to);
        if !self.edges.remove(&(to, from)) {
            self.edges.insert((from, to));
        }
    }

    /** close contours that reach the outer edge of the sampled grid */
    pub fn border(&mut self, from: Vertex, to: Vertex, crossing: Vertex, a: u8, b: u8) {
        match (a, b) {
            (1, 1) => self.edge(from, to),
            (1, 0) => self.edge(from, crossing),
            (0, 1) => self.edge(crossing, to),
            _ => {}
        }
    }

    /** trace directed boundaries with the filled region on their left */
    pub fn finish(self) -> Vec<Vec<Point2D>> {
        let mut edges: Vec<_> = self.edges.into_iter().collect();
        // stable traversal keeps repeated exports deterministic
        edges.sort_unstable();
        let mut outgoing = vec![Vec::new(); self.points.len()];
        for (index, &(from, _)) in edges.iter().enumerate() {
            outgoing[from].push(index);
        }
        let mut visited = vec![false; edges.len()];
        let mut path_index = vec![usize::MAX; self.points.len()];
        let mut path = Vec::new();
        let mut walk = Vec::new();
        let mut rings = Vec::new();
        for first in 0..edges.len() {
            if visited[first] {
                continue;
            }
            let start = edges[first].0;
            let mut edge = first;
            walk.clear();
            loop {
                visited[edge] = true;
                let (from, to) = edges[edge];
                walk.push(from);
                if to == start {
                    break;
                }
                let next = outgoing[to].iter().copied().filter(|&i| !visited[i]);
                // threshold plateaus can make several boundaries meet at one vertex
                let next = next.min_by(|&a, &b| {
                    let reverse = (self.points[from].y - self.points[to].y)
                        .atan2(self.points[from].x - self.points[to].x);
                    let turn = |i: usize| {
                        let end = self.points[edges[i].1];
                        let angle = (end.y - self.points[to].y).atan2(end.x - self.points[to].x);
                        (reverse - angle).rem_euclid(std::f64::consts::TAU)
                    };
                    turn(a).total_cmp(&turn(b))
                });
                let Some(next) = next else {
                    debug_assert!(false, "open contour boundary");
                    walk.clear();
                    break;
                };
                edge = next;
            }
            if walk.is_empty() {
                continue;
            }
            walk.push(start);
            // split pinched walks so touching holes remain separate simple rings
            for &vertex in &walk {
                let split = path_index[vertex];
                if split != usize::MAX {
                    let ring = simplify(path[split..].iter().map(|&i| self.points[i]).collect());
                    if ring.len() >= 4 && signed_area(&ring) != 0.0 {
                        rings.push(ring);
                    }
                    for &i in &path[split..] {
                        path_index[i] = usize::MAX;
                    }
                    path.truncate(split);
                }
                path_index[vertex] = path.len();
                path.push(vertex);
            }
            for vertex in path.drain(..) {
                path_index[vertex] = usize::MAX;
            }
        }
        rings
    }
}

/** remove straight intermediate vertices and close the GeoJSON ring */
fn simplify(ring: Vec<Point2D>) -> Vec<Point2D> {
    if ring.len() < 3 {
        return Vec::new();
    }
    let mut result = Vec::new();
    for i in 0..ring.len() {
        let a = ring[(i + ring.len() - 1) % ring.len()];
        let b = ring[i];
        let c = ring[(i + 1) % ring.len()];
        if (b.x - a.x) * (c.y - b.y) != (b.y - a.y) * (c.x - b.x) {
            result.push(b);
        }
    }
    if let Some(&first) = result.first() {
        result.push(first);
    }
    result
}

pub(super) fn signed_area(ring: &[Point2D]) -> f64 {
    let Some(origin) = ring.first() else {
        return 0.0;
    };
    ring.windows(2)
        .map(|pair| {
            (pair[0].x - origin.x) * (pair[1].y - origin.y)
                - (pair[1].x - origin.x) * (pair[0].y - origin.y)
        })
        .sum::<f64>()
        * 0.5
}

pub(super) fn contains(ring: &[Point2D], point: Point2D) -> bool {
    let mut inside = false;
    for pair in ring.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        if (a.y > point.y) != (b.y > point.y)
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
    }
    inside
}

/** attach each clockwise hole to the smallest containing shell */
pub(super) fn polygons(rings: Vec<Vec<Point2D>>) -> Vec<Vec<Vec<Point2D>>> {
    let mut shells = Vec::new();
    let mut holes = Vec::new();
    for ring in rings {
        let area = signed_area(&ring);
        if area > 0.0 {
            let bounds = ring.iter().fold(
                [
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ],
                |b, p| [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)],
            );
            shells.push((area, bounds, vec![ring]));
        } else if area < 0.0 {
            holes.push(ring);
        }
    }
    let mut order: Vec<_> = (0..shells.len()).collect();
    order.sort_by(|&a, &b| shells[a].0.total_cmp(&shells[b].0));
    for hole in holes {
        // an edge midpoint avoids ambiguous vertices where rings touch
        let sample = Point2D {
            x: (hole[0].x + hole[1].x) * 0.5,
            y: (hole[0].y + hole[1].y) * 0.5,
        };
        let parent = order.iter().copied().find(|&i| {
            let (_, b, rings) = &shells[i];
            sample.x >= b[0]
                && sample.y >= b[1]
                && sample.x <= b[2]
                && sample.y <= b[3]
                && contains(&rings[0], sample)
        });
        if let Some(parent) = parent {
            shells[parent].2.push(hole);
        } else {
            debug_assert!(false, "contour hole without a shell");
        }
    }
    shells.into_iter().map(|(_, _, rings)| rings).collect()
}

/** emit one MultiPolygon feature per level while retaining separate patches and holes */
pub(super) fn feature(rings: Vec<Vec<Point2D>>, concentration: f32) -> serde_json::Value {
    let coordinates: Vec<Vec<Vec<[f64; 2]>>> = polygons(rings)
        .into_iter()
        .map(|polygon| {
            polygon
                .into_iter()
                .map(|ring| ring.into_iter().map(|p| [p.x, p.y]).collect())
                .collect()
        })
        .collect();
    serde_json::json!({
        "type": "Feature",
        "geometry": { "type": "MultiPolygon", "coordinates": coordinates },
        "properties": { "concentration": concentration }
    })
}
