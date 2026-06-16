//! Route Inspection (Chinese Postman Problem) solver.
//!
//! Given a set of street segments, builds the underlying street graph
//! (segments are edges, shared endpoints are intersections/nodes), then
//! computes a walk that covers every edge at least once with a minimal
//! amount of repeated ("deadhead") distance:
//!
//! 1. Snap segment endpoints to merge them into shared intersection nodes.
//! 2. Split the graph into connected components.
//! 3. For each component, find the odd-degree nodes and greedily pair them
//!    up by shortest-path distance (Dijkstra), duplicating the edges along
//!    each shortest path so every node has even degree (a T-join). This is
//!    a greedy approximation of the minimum-weight perfect matching used by
//!    the textbook Chinese Postman algorithm — exact matching requires the
//!    Blossom algorithm, which is significantly more code for a build-time
//!    tool operating on a few thousand edges at most.
//! 4. Run Hierholzer's algorithm on the now-Eulerian component to obtain a
//!    circuit covering every edge (including duplicates).
//! 5. Concatenate the per-component circuits, bridging disconnected
//!    components with a straight-line "transfer" segment.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Coordinates are snapped to this many fractional digits (~1m at the
/// equator) so that shared intersections compare equal even if computed via
/// slightly different floating-point paths.
const COORD_PRECISION: f64 = 1e5;
const EARTH_RADIUS_KM: f64 = 6371.0;

pub struct StreetSegment {
    pub nodes: Vec<(f64, f64)>,
    pub length: f64,
    /// OSM node id of the first/last node, when available. Using stable node
    /// ids makes intersections exact; segments without ids (e.g. synthetic
    /// data) fall back to snapping their endpoint coordinates.
    pub from_id: Option<i64>,
    pub to_id: Option<i64>,
}

/// Identity of a graph vertex: an exact OSM node id when known, otherwise the
/// snapped endpoint coordinate.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum NodeKey {
    Id(i64),
    Coord(i64, i64),
}

pub struct RouteResult {
    pub coordinates: Vec<(f64, f64)>,
    pub total_distance_km: f64,
}

#[derive(Clone)]
struct Edge {
    street_index: usize,
    from: usize,
    to: usize,
    weight: f64,
}

pub fn optimize_route(streets: &[StreetSegment]) -> RouteResult {
    if streets.is_empty() {
        return RouteResult {
            coordinates: Vec::new(),
            total_distance_km: 0.0,
        };
    }

    let (node_coords, edges) = build_graph(streets);
    let mut components = connected_components(node_coords.len(), &edges);
    // Largest (by total length) component first, so the main street network
    // leads the route and small/isolated streets are appended afterwards.
    components.sort_by(|a, b| {
        let weight_a: f64 = a.iter().map(|&e| edges[e].weight).sum();
        let weight_b: f64 = b.iter().map(|&e| edges[e].weight).sum();
        weight_b.partial_cmp(&weight_a).unwrap_or(Ordering::Equal)
    });

    let mut coordinates: Vec<(f64, f64)> = Vec::new();
    let mut total_distance_km = 0.0;

    for component_edges in components {
        let (circuit, component_distance) =
            solve_component(node_coords.len(), &edges, &component_edges);
        total_distance_km += component_distance;

        for (street_index, reversed) in circuit {
            let street = &streets[street_index];
            if let Some(&last) = coordinates.last() {
                let first = if reversed {
                    *street.nodes.last().expect("street has nodes")
                } else {
                    street.nodes[0]
                };
                total_distance_km += haversine_km(last, first);
            }
            if reversed {
                coordinates.extend(street.nodes.iter().rev().copied());
            } else {
                coordinates.extend(street.nodes.iter().copied());
            }
        }
    }

    RouteResult {
        coordinates,
        total_distance_km,
    }
}

/// Build the street graph: nodes are snapped segment endpoints, edges are
/// street segments connecting them.
fn build_graph(streets: &[StreetSegment]) -> (Vec<(f64, f64)>, Vec<Edge>) {
    let mut node_index: HashMap<NodeKey, usize> = HashMap::new();
    let mut node_coords: Vec<(f64, f64)> = Vec::new();
    let mut edges = Vec::with_capacity(streets.len());

    for (street_index, street) in streets.iter().enumerate() {
        let first = street.nodes[0];
        let last = *street.nodes.last().expect("street has nodes");
        let from = get_or_insert_node(
            node_key(street.from_id, first),
            first,
            &mut node_index,
            &mut node_coords,
        );
        let to = get_or_insert_node(
            node_key(street.to_id, last),
            last,
            &mut node_index,
            &mut node_coords,
        );
        edges.push(Edge {
            street_index,
            from,
            to,
            weight: street.length,
        });
    }

    (node_coords, edges)
}

fn node_key(id: Option<i64>, coord: (f64, f64)) -> NodeKey {
    match id {
        Some(id) => NodeKey::Id(id),
        None => {
            let (lat, lng) = snap(coord);
            NodeKey::Coord(lat, lng)
        }
    }
}

fn get_or_insert_node(
    key: NodeKey,
    coord: (f64, f64),
    node_index: &mut HashMap<NodeKey, usize>,
    node_coords: &mut Vec<(f64, f64)>,
) -> usize {
    if let Some(&index) = node_index.get(&key) {
        return index;
    }
    let index = node_coords.len();
    node_coords.push(coord);
    node_index.insert(key, index);
    index
}

fn snap(coord: (f64, f64)) -> (i64, i64) {
    (
        (coord.0 * COORD_PRECISION).round() as i64,
        (coord.1 * COORD_PRECISION).round() as i64,
    )
}

/// Group edges into connected components (by shared nodes) using a
/// disjoint-set over node indices.
fn connected_components(node_count: usize, edges: &[Edge]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..node_count).collect();

    fn find(parent: &mut [usize], x: usize) -> usize {
        if parent[x] != x {
            parent[x] = find(parent, parent[x]);
        }
        parent[x]
    }

    for edge in edges {
        let root_from = find(&mut parent, edge.from);
        let root_to = find(&mut parent, edge.to);
        if root_from != root_to {
            parent[root_from] = root_to;
        }
    }

    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for (edge_index, edge) in edges.iter().enumerate() {
        let root = find(&mut parent, edge.from);
        groups.entry(root).or_default().push(edge_index);
    }

    groups.into_values().collect()
}

fn build_adjacency(node_count: usize, edges: &[Edge]) -> Vec<Vec<usize>> {
    let mut adjacency = vec![Vec::new(); node_count];
    for (edge_index, edge) in edges.iter().enumerate() {
        adjacency[edge.from].push(edge_index);
        adjacency[edge.to].push(edge_index);
    }
    adjacency
}

/// Make every node in a component even-degree by duplicating the edges of
/// shortest paths between greedily-paired odd-degree nodes, then return an
/// Eulerian circuit covering every edge (including duplicates) and its total
/// distance.
fn solve_component(
    node_count: usize,
    base_edges: &[Edge],
    component_edge_indices: &[usize],
) -> (Vec<(usize, bool)>, f64) {
    let mut edges: Vec<Edge> = component_edge_indices
        .iter()
        .map(|&i| base_edges[i].clone())
        .collect();
    let mut adjacency = build_adjacency(node_count, &edges);

    let mut odd_nodes: Vec<usize> = adjacency
        .iter()
        .enumerate()
        .filter(|(_, adj)| adj.len() % 2 == 1)
        .map(|(node, _)| node)
        .collect();

    while odd_nodes.len() >= 2 {
        let source = odd_nodes.remove(0);
        let (dist, prev_edge) = dijkstra(source, node_count, &edges, &adjacency);

        let (best_pos, &target) = odd_nodes
            .iter()
            .enumerate()
            .min_by(|(_, &a), (_, &b)| dist[a].partial_cmp(&dist[b]).unwrap_or(Ordering::Equal))
            .expect("at least one remaining odd node");
        odd_nodes.remove(best_pos);

        for edge_index in reconstruct_path(target, source, &edges, &prev_edge) {
            let edge = edges[edge_index].clone();
            let new_index = edges.len();
            adjacency[edge.from].push(new_index);
            adjacency[edge.to].push(new_index);
            edges.push(edge);
        }
    }

    let start = edges[0].from;
    let circuit = hierholzer(start, &edges, &adjacency);
    let total_distance: f64 = circuit.iter().map(|&(e, _)| edges[e].weight).sum();

    let circuit = circuit
        .into_iter()
        .map(|(edge_index, reversed)| (edges[edge_index].street_index, reversed))
        .collect();

    (circuit, total_distance)
}

#[derive(PartialEq)]
struct DijkstraState {
    cost: f64,
    node: usize,
}

impl Eq for DijkstraState {}

impl Ord for DijkstraState {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed so `BinaryHeap` (a max-heap) behaves as a min-heap.
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for DijkstraState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn dijkstra(
    source: usize,
    node_count: usize,
    edges: &[Edge],
    adjacency: &[Vec<usize>],
) -> (Vec<f64>, Vec<Option<usize>>) {
    let mut dist = vec![f64::INFINITY; node_count];
    let mut prev_edge: Vec<Option<usize>> = vec![None; node_count];
    let mut heap = BinaryHeap::new();

    dist[source] = 0.0;
    heap.push(DijkstraState {
        cost: 0.0,
        node: source,
    });

    while let Some(DijkstraState { cost, node }) = heap.pop() {
        if cost > dist[node] {
            continue;
        }
        for &edge_index in &adjacency[node] {
            let edge = &edges[edge_index];
            if edge.from == edge.to {
                continue; // self-loops never shorten a path to another node
            }
            let next = if edge.from == node {
                edge.to
            } else {
                edge.from
            };
            let new_cost = cost + edge.weight;
            if new_cost < dist[next] {
                dist[next] = new_cost;
                prev_edge[next] = Some(edge_index);
                heap.push(DijkstraState {
                    cost: new_cost,
                    node: next,
                });
            }
        }
    }

    (dist, prev_edge)
}

fn reconstruct_path(
    target: usize,
    source: usize,
    edges: &[Edge],
    prev_edge: &[Option<usize>],
) -> Vec<usize> {
    let mut path = Vec::new();
    let mut current = target;
    while current != source {
        let edge_index = prev_edge[current].expect("path exists between matched odd nodes");
        let edge = &edges[edge_index];
        current = if edge.to == current {
            edge.from
        } else {
            edge.to
        };
        path.push(edge_index);
    }
    path.reverse();
    path
}

/// Hierholzer's algorithm: returns the edges of an Eulerian circuit starting
/// at `start`, in traversal order, alongside whether each edge is traversed
/// in reverse of its `(from, to)` direction.
fn hierholzer(start: usize, edges: &[Edge], adjacency: &[Vec<usize>]) -> Vec<(usize, bool)> {
    let mut used = vec![false; edges.len()];
    let mut pointer = vec![0usize; adjacency.len()];
    let mut vertex_stack = vec![start];
    let mut edge_stack: Vec<usize> = Vec::new();
    let mut circuit: Vec<usize> = Vec::new();

    while let Some(&vertex) = vertex_stack.last() {
        let mut advanced = false;
        while pointer[vertex] < adjacency[vertex].len() {
            let edge_index = adjacency[vertex][pointer[vertex]];
            pointer[vertex] += 1;
            if !used[edge_index] {
                used[edge_index] = true;
                let edge = &edges[edge_index];
                let next = if edge.from == vertex {
                    edge.to
                } else {
                    edge.from
                };
                vertex_stack.push(next);
                edge_stack.push(edge_index);
                advanced = true;
                break;
            }
        }
        if !advanced {
            vertex_stack.pop();
            if let Some(edge_index) = edge_stack.pop() {
                circuit.push(edge_index);
            }
        }
    }

    circuit.reverse();

    let mut current = start;
    circuit
        .into_iter()
        .map(|edge_index| {
            let edge = &edges[edge_index];
            let reversed = edge.from != current;
            current = if reversed { edge.from } else { edge.to };
            (edge_index, reversed)
        })
        .collect()
}

fn haversine_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (lat1, lon1) = a;
    let (lat2, lon2) = b;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let lat1 = lat1.to_radians();
    let lat2 = lat2.to_radians();

    let sin_dlat = (d_lat / 2.0).sin();
    let sin_dlon = (d_lon / 2.0).sin();
    let a = sin_dlat * sin_dlat + lat1.cos() * lat2.cos() * sin_dlon * sin_dlon;
    let c = 2.0 * a.sqrt().asin();

    EARTH_RADIUS_KM * c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(nodes: Vec<(f64, f64)>) -> StreetSegment {
        let length = nodes
            .windows(2)
            .map(|pair| haversine_km(pair[0], pair[1]))
            .sum();
        StreetSegment {
            nodes,
            length,
            from_id: None,
            to_id: None,
        }
    }

    #[test]
    fn empty_input_produces_empty_route() {
        let result = optimize_route(&[]);
        assert!(result.coordinates.is_empty());
        assert_eq!(result.total_distance_km, 0.0);
    }

    #[test]
    fn eulerian_square_is_covered_without_repeats() {
        // A 1x1 square: every node has degree 2 (even), so the Chinese
        // Postman tour is a plain Eulerian circuit with no duplicated edges.
        let a = (48.0, 2.0);
        let b = (48.0, 2.001);
        let c = (48.001, 2.001);
        let d = (48.001, 2.0);

        let streets = vec![
            segment(vec![a, b]),
            segment(vec![b, c]),
            segment(vec![c, d]),
            segment(vec![d, a]),
        ];
        let expected_total: f64 = streets.iter().map(|s| s.length).sum();

        let result = optimize_route(&streets);

        // Each street's two endpoints appear exactly once in the route.
        assert_eq!(result.coordinates.len(), 8);
        assert!((result.total_distance_km - expected_total).abs() < 1e-9);
    }

    #[test]
    fn dead_end_street_is_traversed_there_and_back() {
        // A single isolated segment: both endpoints have degree 1 (odd), so
        // the only way to cover it is to walk it twice.
        let a = (48.0, 2.0);
        let b = (48.001, 2.0);
        let streets = vec![segment(vec![a, b])];
        let expected_total = streets[0].length * 2.0;

        let result = optimize_route(&streets);

        assert_eq!(result.coordinates.len(), 4);
        assert!((result.total_distance_km - expected_total).abs() < 1e-9);
    }

    fn id_segment(nodes: Vec<(f64, f64)>, from_id: i64, to_id: i64) -> StreetSegment {
        let length = nodes
            .windows(2)
            .map(|pair| haversine_km(pair[0], pair[1]))
            .sum();
        StreetSegment {
            nodes,
            length,
            from_id: Some(from_id),
            to_id: Some(to_id),
        }
    }

    #[test]
    fn segments_are_joined_by_shared_node_ids() {
        // A triangle whose vertices are identified by OSM node ids 1/2/3.
        // Connectivity comes purely from the shared ids, so the three edges
        // form a single even-degree (Eulerian) circuit covered exactly once.
        let a = (48.0, 2.0);
        let b = (48.0, 2.001);
        let c = (48.001, 2.0005);
        let streets = vec![
            id_segment(vec![a, b], 1, 2),
            id_segment(vec![b, c], 2, 3),
            id_segment(vec![c, a], 3, 1),
        ];
        let expected: f64 = streets.iter().map(|s| s.length).sum();

        let result = optimize_route(&streets);

        assert_eq!(result.coordinates.len(), 6);
        assert!((result.total_distance_km - expected).abs() < 1e-9);
    }

    #[test]
    fn disconnected_components_are_bridged() {
        // Two isolated segments far apart: each is traversed there and back,
        // plus a transfer between them.
        let a = (48.0, 2.0);
        let b = (48.001, 2.0);
        let c = (49.0, 3.0);
        let d = (49.001, 3.0);
        let streets = vec![segment(vec![a, b]), segment(vec![c, d])];

        let result = optimize_route(&streets);

        let streets_total: f64 = streets.iter().map(|s| s.length * 2.0).sum();
        // The result must include at least the two out-and-back traversals
        // plus a non-zero transfer between the disconnected components.
        assert!(result.total_distance_km > streets_total);
        assert_eq!(result.coordinates.len(), 8);
    }
}
