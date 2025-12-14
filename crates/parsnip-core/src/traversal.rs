//! Graph traversal types and algorithms

use crate::entity::Entity;
use crate::relation::{Direction, Relation};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

/// Traversal query builder (follows SearchQuery pattern)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalQuery {
    /// Starting entity name
    pub start: String,

    /// Target entity name (for path finding, None for general traversal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// Maximum traversal depth
    #[serde(default = "default_depth")]
    pub max_depth: u32,

    /// Traversal direction
    #[serde(default)]
    pub direction: Direction,

    /// Filter by entity types (empty = all types)
    #[serde(default)]
    pub entity_type_filter: Vec<String>,

    /// Filter by relation types (empty = all types)
    #[serde(default)]
    pub relation_type_filter: Vec<String>,

    /// Use weighted shortest path (Dijkstra)
    #[serde(default)]
    pub use_weights: bool,

    /// Return all paths (not just shortest)
    #[serde(default)]
    pub all_paths: bool,

    /// Maximum paths to return
    #[serde(default = "default_max_paths")]
    pub max_paths: usize,
}

fn default_depth() -> u32 {
    10
}

fn default_max_paths() -> usize {
    5
}

impl Default for TraversalQuery {
    fn default() -> Self {
        Self {
            start: String::new(),
            target: None,
            max_depth: default_depth(),
            direction: Direction::Both,
            entity_type_filter: Vec::new(),
            relation_type_filter: Vec::new(),
            use_weights: false,
            all_paths: false,
            max_paths: default_max_paths(),
        }
    }
}

impl TraversalQuery {
    /// Create a new traversal query starting from an entity
    pub fn new(start: impl Into<String>) -> Self {
        Self {
            start: start.into(),
            ..Default::default()
        }
    }

    /// Set target for path finding
    pub fn find_path_to(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Set maximum traversal depth
    pub fn with_depth(mut self, depth: u32) -> Self {
        self.max_depth = depth;
        self
    }

    /// Set traversal direction
    pub fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    /// Filter by entity types during traversal
    pub fn filter_entity_types(mut self, types: Vec<String>) -> Self {
        self.entity_type_filter = types;
        self
    }

    /// Filter by relation types during traversal
    pub fn filter_relation_types(mut self, types: Vec<String>) -> Self {
        self.relation_type_filter = types;
        self
    }

    /// Use weighted shortest path (Dijkstra algorithm)
    pub fn weighted(mut self) -> Self {
        self.use_weights = true;
        self
    }

    /// Return all paths up to max
    pub fn all_paths(mut self, max: usize) -> Self {
        self.all_paths = true;
        self.max_paths = max;
        self
    }
}

/// A single path through the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPath {
    /// Ordered list of entity names in the path
    pub nodes: Vec<String>,

    /// Relations connecting the nodes
    pub edges: Vec<PathEdge>,

    /// Total path weight (sum of relation weights, 1.0 for unweighted)
    pub total_weight: f64,

    /// Path length (number of edges)
    pub length: usize,
}

/// Edge in a path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathEdge {
    pub from: String,
    pub to: String,
    pub relation_type: String,
    pub weight: Option<f64>,
}

/// Result of a traversal operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalResult {
    /// Starting entity
    pub start: String,

    /// Target entity (if path finding)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// Found paths (for path finding)
    pub paths: Vec<GraphPath>,

    /// Visited entities (for general traversal)
    pub visited_entities: Vec<String>,

    /// All entities in result
    pub entities: Vec<Entity>,

    /// All relations in result
    pub relations: Vec<Relation>,

    /// Statistics
    pub stats: TraversalStats,
}

/// Traversal statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraversalStats {
    pub nodes_visited: usize,
    pub edges_traversed: usize,
    pub max_depth_reached: u32,
    pub path_found: bool,
}

/// State for Dijkstra priority queue
#[derive(Clone, PartialEq)]
struct DijkstraState {
    cost: f64,
    node: String,
}

impl Eq for DijkstraState {}

impl Ord for DijkstraState {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min-heap
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

/// Graph traversal engine
pub struct TraversalEngine;

impl TraversalEngine {
    /// Execute a traversal query
    pub fn execute(
        query: &TraversalQuery,
        entities: &HashMap<String, Entity>,
        relations: &[Relation],
    ) -> TraversalResult {
        tracing::debug!(
            "Executing traversal: start={}, target={:?}, depth={}, direction={:?}",
            query.start,
            query.target,
            query.max_depth,
            query.direction
        );

        if query.target.is_some() {
            if query.use_weights {
                Self::dijkstra_path(query, entities, relations)
            } else {
                Self::bfs_path(query, entities, relations)
            }
        } else {
            Self::filtered_bfs(query, entities, relations)
        }
    }

    /// BFS for unweighted shortest path
    fn bfs_path(
        query: &TraversalQuery,
        entities: &HashMap<String, Entity>,
        relations: &[Relation],
    ) -> TraversalResult {
        let target = query.target.as_ref().unwrap();
        let mut visited: HashSet<String> = HashSet::new();
        let mut parent: HashMap<String, (String, PathEdge)> = HashMap::new();
        let mut queue: VecDeque<(String, u32)> = VecDeque::new();
        let mut stats = TraversalStats::default();

        queue.push_back((query.start.clone(), 0));
        visited.insert(query.start.clone());

        while let Some((current, depth)) = queue.pop_front() {
            stats.nodes_visited += 1;
            stats.max_depth_reached = stats.max_depth_reached.max(depth);

            if &current == target {
                stats.path_found = true;
                tracing::debug!("BFS found path at depth {}", depth);
                break;
            }

            if depth >= query.max_depth {
                continue;
            }

            for rel in Self::get_neighbors(&current, &query.direction, relations) {
                stats.edges_traversed += 1;

                // Apply relation type filter
                if !query.relation_type_filter.is_empty()
                    && !query.relation_type_filter.contains(&rel.relation_type)
                {
                    continue;
                }

                let next = if rel.from_name == current {
                    &rel.to_name
                } else {
                    &rel.from_name
                };

                // Apply entity type filter
                if let Some(entity) = entities.get(next) {
                    if !query.entity_type_filter.is_empty()
                        && !query.entity_type_filter.contains(&entity.entity_type.0)
                    {
                        continue;
                    }
                }

                if !visited.contains(next) {
                    visited.insert(next.clone());
                    parent.insert(
                        next.clone(),
                        (
                            current.clone(),
                            PathEdge {
                                from: rel.from_name.clone(),
                                to: rel.to_name.clone(),
                                relation_type: rel.relation_type.clone(),
                                weight: rel.weight,
                            },
                        ),
                    );
                    queue.push_back((next.clone(), depth + 1));
                }
            }
        }

        // Reconstruct path
        let paths = if stats.path_found {
            vec![Self::reconstruct_path(&query.start, target, &parent)]
        } else {
            vec![]
        };

        Self::build_result(query, paths, &visited, entities, relations, stats)
    }

    /// Dijkstra's algorithm for weighted shortest path
    fn dijkstra_path(
        query: &TraversalQuery,
        entities: &HashMap<String, Entity>,
        relations: &[Relation],
    ) -> TraversalResult {
        let target = query.target.as_ref().unwrap();
        let mut dist: HashMap<String, f64> = HashMap::new();
        let mut parent: HashMap<String, (String, PathEdge)> = HashMap::new();
        let mut heap = BinaryHeap::new();
        let mut stats = TraversalStats::default();

        dist.insert(query.start.clone(), 0.0);
        heap.push(DijkstraState {
            cost: 0.0,
            node: query.start.clone(),
        });

        while let Some(DijkstraState { cost, node }) = heap.pop() {
            stats.nodes_visited += 1;

            if &node == target {
                stats.path_found = true;
                tracing::debug!("Dijkstra found path with cost {}", cost);
                break;
            }

            // Skip if we already found a better path
            if cost > *dist.get(&node).unwrap_or(&f64::INFINITY) {
                continue;
            }

            for rel in Self::get_neighbors(&node, &query.direction, relations) {
                stats.edges_traversed += 1;

                // Apply relation type filter
                if !query.relation_type_filter.is_empty()
                    && !query.relation_type_filter.contains(&rel.relation_type)
                {
                    continue;
                }

                let next = if rel.from_name == node {
                    &rel.to_name
                } else {
                    &rel.from_name
                };

                // Apply entity type filter
                if let Some(entity) = entities.get(next) {
                    if !query.entity_type_filter.is_empty()
                        && !query.entity_type_filter.contains(&entity.entity_type.0)
                    {
                        continue;
                    }
                }

                let edge_weight = rel.weight.unwrap_or(1.0);
                let new_cost = cost + edge_weight;

                if new_cost < *dist.get(next).unwrap_or(&f64::INFINITY) {
                    dist.insert(next.clone(), new_cost);
                    parent.insert(
                        next.clone(),
                        (
                            node.clone(),
                            PathEdge {
                                from: rel.from_name.clone(),
                                to: rel.to_name.clone(),
                                relation_type: rel.relation_type.clone(),
                                weight: rel.weight,
                            },
                        ),
                    );
                    heap.push(DijkstraState {
                        cost: new_cost,
                        node: next.clone(),
                    });
                }
            }
        }

        let paths = if stats.path_found {
            vec![Self::reconstruct_path(&query.start, target, &parent)]
        } else {
            vec![]
        };

        let visited: HashSet<String> = dist.keys().cloned().collect();
        Self::build_result(query, paths, &visited, entities, relations, stats)
    }

    /// Filtered BFS traversal (no target)
    fn filtered_bfs(
        query: &TraversalQuery,
        entities: &HashMap<String, Entity>,
        relations: &[Relation],
    ) -> TraversalResult {
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, u32)> = VecDeque::new();
        let mut stats = TraversalStats::default();

        queue.push_back((query.start.clone(), 0));
        visited.insert(query.start.clone());

        while let Some((current, depth)) = queue.pop_front() {
            stats.nodes_visited += 1;
            stats.max_depth_reached = stats.max_depth_reached.max(depth);

            if depth >= query.max_depth {
                continue;
            }

            for rel in Self::get_neighbors(&current, &query.direction, relations) {
                stats.edges_traversed += 1;

                // Apply relation type filter
                if !query.relation_type_filter.is_empty()
                    && !query.relation_type_filter.contains(&rel.relation_type)
                {
                    continue;
                }

                let next = if rel.from_name == current {
                    &rel.to_name
                } else {
                    &rel.from_name
                };

                // Apply entity type filter
                if let Some(entity) = entities.get(next) {
                    if !query.entity_type_filter.is_empty()
                        && !query.entity_type_filter.contains(&entity.entity_type.0)
                    {
                        continue;
                    }
                }

                if !visited.contains(next) {
                    visited.insert(next.clone());
                    queue.push_back((next.clone(), depth + 1));
                }
            }
        }

        tracing::debug!(
            "Filtered BFS visited {} nodes, traversed {} edges",
            stats.nodes_visited,
            stats.edges_traversed
        );

        Self::build_result(query, vec![], &visited, entities, relations, stats)
    }

    /// Get neighboring relations for a node based on direction
    fn get_neighbors<'a>(
        node: &str,
        direction: &Direction,
        relations: &'a [Relation],
    ) -> Vec<&'a Relation> {
        relations
            .iter()
            .filter(|rel| match direction {
                Direction::Outgoing => rel.from_name == node,
                Direction::Incoming => rel.to_name == node,
                Direction::Both => rel.from_name == node || rel.to_name == node,
            })
            .collect()
    }

    /// Reconstruct path from parent map
    fn reconstruct_path(
        start: &str,
        end: &str,
        parent: &HashMap<String, (String, PathEdge)>,
    ) -> GraphPath {
        let mut nodes = vec![end.to_string()];
        let mut edges = Vec::new();
        let mut current = end.to_string();
        let mut total_weight = 0.0;

        while current != start {
            if let Some((prev, edge)) = parent.get(&current) {
                total_weight += edge.weight.unwrap_or(1.0);
                edges.push(edge.clone());
                nodes.push(prev.clone());
                current = prev.clone();
            } else {
                break;
            }
        }

        nodes.reverse();
        edges.reverse();

        GraphPath {
            length: edges.len(),
            nodes,
            edges,
            total_weight,
        }
    }

    /// Build result from traversal data
    fn build_result(
        query: &TraversalQuery,
        paths: Vec<GraphPath>,
        visited: &HashSet<String>,
        entities: &HashMap<String, Entity>,
        relations: &[Relation],
        stats: TraversalStats,
    ) -> TraversalResult {
        let visited_entities: Vec<String> = visited.iter().cloned().collect();

        let result_entities: Vec<Entity> = visited_entities
            .iter()
            .filter_map(|name| entities.get(name).cloned())
            .collect();

        let result_relations: Vec<Relation> = relations
            .iter()
            .filter(|r| visited.contains(&r.from_name) && visited.contains(&r.to_name))
            .cloned()
            .collect();

        TraversalResult {
            start: query.start.clone(),
            target: query.target.clone(),
            paths,
            visited_entities,
            entities: result_entities,
            relations: result_relations,
            stats,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectId;

    fn create_test_graph() -> (HashMap<String, Entity>, Vec<Relation>) {
        let project_id = ProjectId::new();

        // Create entities: A, B, C, D, E, F
        let mut entities = HashMap::new();
        for name in ["A", "B", "C", "D", "E", "F"] {
            let entity = Entity::new(project_id.clone(), name, "node");
            entities.insert(name.to_string(), entity);
        }

        // Create graph:
        // A --1.0--> B --2.0--> C --1.0--> D
        //            |          |
        //            v          v
        //            E --3.0--> F
        let relations = vec![
            Relation::from_names(project_id.clone(), "A", "B", "connects").with_weight(1.0),
            Relation::from_names(project_id.clone(), "B", "C", "connects").with_weight(2.0),
            Relation::from_names(project_id.clone(), "C", "D", "connects").with_weight(1.0),
            Relation::from_names(project_id.clone(), "B", "E", "connects").with_weight(1.0),
            Relation::from_names(project_id.clone(), "C", "F", "connects").with_weight(1.0),
            Relation::from_names(project_id.clone(), "E", "F", "connects").with_weight(3.0),
        ];

        (entities, relations)
    }

    #[test]
    fn test_bfs_shortest_path() {
        let (entities, relations) = create_test_graph();
        let query = TraversalQuery::new("A").find_path_to("D");
        let result = TraversalEngine::execute(&query, &entities, &relations);

        assert!(result.stats.path_found);
        assert_eq!(result.paths.len(), 1);
        assert_eq!(result.paths[0].nodes, vec!["A", "B", "C", "D"]);
        assert_eq!(result.paths[0].length, 3);
    }

    #[test]
    fn test_dijkstra_weighted_path() {
        let (entities, relations) = create_test_graph();

        // Find path from A to F
        // Path via C: A->B->C->F = 1+2+1 = 4
        // Path via E: A->B->E->F = 1+1+3 = 5
        // Dijkstra should find A->B->C->F with weight 4
        let query = TraversalQuery::new("A").find_path_to("F").weighted();
        let result = TraversalEngine::execute(&query, &entities, &relations);

        assert!(result.stats.path_found);
        assert_eq!(result.paths[0].nodes, vec!["A", "B", "C", "F"]);
        assert!((result.paths[0].total_weight - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_filtered_traversal() {
        let (entities, relations) = create_test_graph();
        let query = TraversalQuery::new("A").with_depth(2);
        let result = TraversalEngine::execute(&query, &entities, &relations);

        // At depth 2 from A: A(0), B(1), C(2), E(2)
        assert!(result.visited_entities.contains(&"A".to_string()));
        assert!(result.visited_entities.contains(&"B".to_string()));
        assert!(result.visited_entities.contains(&"C".to_string()));
        assert!(result.visited_entities.contains(&"E".to_string()));
    }

    #[test]
    fn test_no_path_found() {
        let project_id = ProjectId::new();
        let mut entities = HashMap::new();
        entities.insert(
            "A".to_string(),
            Entity::new(project_id.clone(), "A", "node"),
        );
        entities.insert(
            "B".to_string(),
            Entity::new(project_id.clone(), "B", "node"),
        );
        // No relations - disconnected graph

        let query = TraversalQuery::new("A").find_path_to("B");
        let result = TraversalEngine::execute(&query, &entities, &[]);

        assert!(!result.stats.path_found);
        assert!(result.paths.is_empty());
    }

    #[test]
    fn test_direction_filtering() {
        let (entities, relations) = create_test_graph();

        // Outgoing from B should reach C, E
        let outgoing = TraversalQuery::new("B")
            .with_direction(Direction::Outgoing)
            .with_depth(1);
        let result = TraversalEngine::execute(&outgoing, &entities, &relations);
        assert!(result.visited_entities.contains(&"C".to_string()));
        assert!(result.visited_entities.contains(&"E".to_string()));
        assert!(!result.visited_entities.contains(&"A".to_string()));

        // Incoming to B should only reach A
        let incoming = TraversalQuery::new("B")
            .with_direction(Direction::Incoming)
            .with_depth(1);
        let result = TraversalEngine::execute(&incoming, &entities, &relations);
        assert!(result.visited_entities.contains(&"A".to_string()));
        assert!(!result.visited_entities.contains(&"C".to_string()));
    }

    #[test]
    fn test_relation_type_filter() {
        let project_id = ProjectId::new();
        let mut entities = HashMap::new();
        for name in ["A", "B", "C"] {
            entities.insert(
                name.to_string(),
                Entity::new(project_id.clone(), name, "node"),
            );
        }

        let relations = vec![
            Relation::from_names(project_id.clone(), "A", "B", "works_at"),
            Relation::from_names(project_id.clone(), "B", "C", "knows"),
        ];

        // Only follow "works_at" relations
        let query = TraversalQuery::new("A")
            .with_depth(2)
            .filter_relation_types(vec!["works_at".to_string()]);
        let result = TraversalEngine::execute(&query, &entities, &relations);

        // Should reach B but not C
        assert!(result.visited_entities.contains(&"B".to_string()));
        assert!(!result.visited_entities.contains(&"C".to_string()));
    }
}
