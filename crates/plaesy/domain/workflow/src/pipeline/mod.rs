//! Pipeline as a DAG of steps (capability #20, "Workflow DAG dan pipeline").
//!
//! Pure logic, no I/O. A [`Pipeline`] is validated once: step ids are non-empty and
//! unique, every dependency exists, and the graph has no cycle. Its execution order
//! is a topological order. Ties are broken by declaration order, so the same
//! definition always yields the same order.
//!
//! A [`PipelineRun`] keeps one [`RunState`] per step. Each state changes only through
//! [`crate::task_run::transition`], so a step follows the same lifecycle as a task run.
//! Status mapping for a step: `Queued` (menunggu), `Running` (berjalan),
//! `Succeeded` (selesai), `Failed` (gagal).

use std::collections::{BTreeSet, HashMap};
use std::fmt;

use crate::task_run::{transition, Event, RunState, TransitionError};

/// Input for one step: its id and the ids of the steps it depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepSpec {
    pub id: String,
    pub depends_on: Vec<String>,
}

impl StepSpec {
    /// Builds a step spec from string slices.
    #[must_use]
    pub fn new(id: &str, depends_on: &[&str]) -> Self {
        Self {
            id: id.to_owned(),
            depends_on: depends_on.iter().map(|d| (*d).to_owned()).collect(),
        }
    }
}

/// Why a pipeline definition or a step operation was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    /// A step id is empty or whitespace only.
    EmptyId,
    /// Two steps share the same id.
    DuplicateStep { id: String },
    /// A step depends on itself.
    SelfDependency { step: String },
    /// A step depends on an id that is not declared in the pipeline.
    UnknownDependency { step: String, dependency: String },
    /// The dependencies form a cycle. `steps` lists one cycle in dependency order,
    /// where each step depends on the next and the last depends on the first.
    Cycle { steps: Vec<String> },
    /// No step has this id.
    UnknownStep { id: String },
    /// A step was started before one of its dependencies succeeded.
    DependencyNotSatisfied { step: String, dependency: String },
    /// The step's lifecycle refused the event.
    Transition(TransitionError),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => write!(f, "step id must not be empty"),
            Self::DuplicateStep { id } => write!(f, "duplicate step id: {id}"),
            Self::SelfDependency { step } => write!(f, "step {step} depends on itself"),
            Self::UnknownDependency { step, dependency } => {
                write!(f, "step {step} depends on unknown step {dependency}")
            }
            Self::Cycle { steps } => {
                write!(f, "dependency cycle: {}", steps.join(" -> "))?;
                if let Some(first) = steps.first() {
                    write!(f, " -> {first}")?;
                }
                Ok(())
            }
            Self::UnknownStep { id } => write!(f, "unknown step: {id}"),
            Self::DependencyNotSatisfied { step, dependency } => {
                write!(
                    f,
                    "step {step} cannot start: dependency {dependency} has not succeeded"
                )
            }
            Self::Transition(error) => write!(f, "invalid step transition: {error:?}"),
        }
    }
}

impl std::error::Error for PipelineError {}

/// A validated pipeline definition with a fixed topological order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipeline {
    ids: Vec<String>,
    /// For each step index, the indices of the steps it depends on (no duplicates).
    deps: Vec<Vec<usize>>,
    /// Step indices in execution order.
    order: Vec<usize>,
    index: HashMap<String, usize>,
}

impl Pipeline {
    /// Validates the step specs and computes the execution order.
    ///
    /// # Errors
    ///
    /// Returns a [`PipelineError`] for an empty or duplicate id, a self or unknown
    /// dependency, or a dependency cycle.
    pub fn new(specs: &[StepSpec]) -> Result<Self, PipelineError> {
        let mut ids = Vec::with_capacity(specs.len());
        let mut index = HashMap::with_capacity(specs.len());
        for spec in specs {
            if spec.id.trim().is_empty() {
                return Err(PipelineError::EmptyId);
            }
            if index.insert(spec.id.clone(), ids.len()).is_some() {
                return Err(PipelineError::DuplicateStep {
                    id: spec.id.clone(),
                });
            }
            ids.push(spec.id.clone());
        }

        let mut deps: Vec<Vec<usize>> = Vec::with_capacity(specs.len());
        for spec in specs {
            let mut resolved: Vec<usize> = Vec::new();
            for dep in &spec.depends_on {
                if *dep == spec.id {
                    return Err(PipelineError::SelfDependency {
                        step: spec.id.clone(),
                    });
                }
                let Some(&j) = index.get(dep) else {
                    return Err(PipelineError::UnknownDependency {
                        step: spec.id.clone(),
                        dependency: dep.clone(),
                    });
                };
                if !resolved.contains(&j) {
                    resolved.push(j);
                }
            }
            deps.push(resolved);
        }

        // Kahn's algorithm. The ready set is ordered by index, so ties follow declaration order.
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); ids.len()];
        let mut pending: Vec<usize> = deps.iter().map(Vec::len).collect();
        for (i, ds) in deps.iter().enumerate() {
            for &d in ds {
                dependents[d].push(i);
            }
        }
        let mut ready: BTreeSet<usize> = (0..ids.len()).filter(|&i| pending[i] == 0).collect();
        let mut order = Vec::with_capacity(ids.len());
        while let Some(i) = ready.pop_first() {
            order.push(i);
            for &j in &dependents[i] {
                pending[j] = pending[j].saturating_sub(1);
                if pending[j] == 0 {
                    ready.insert(j);
                }
            }
        }

        if order.len() != ids.len() {
            let mut done = vec![false; ids.len()];
            for &i in &order {
                done[i] = true;
            }
            return Err(PipelineError::Cycle {
                steps: find_cycle(&ids, &deps, &done),
            });
        }

        Ok(Self {
            ids,
            deps,
            order,
            index,
        })
    }

    /// Step ids in execution (topological) order.
    #[must_use]
    pub fn order(&self) -> Vec<&str> {
        self.order.iter().map(|&i| self.ids[i].as_str()).collect()
    }

    /// Starts a run in which every step is `Queued`.
    #[must_use]
    pub fn start(&self) -> PipelineRun<'_> {
        PipelineRun {
            pipeline: self,
            states: vec![RunState::Queued; self.ids.len()],
        }
    }

    fn position(&self, id: &str) -> Result<usize, PipelineError> {
        self.index
            .get(id)
            .copied()
            .ok_or_else(|| PipelineError::UnknownStep { id: id.to_owned() })
    }
}

/// Finds one dependency cycle among the steps that are not yet ordered.
///
/// Every unordered step still has an unordered dependency, so walking dependencies
/// from any of them must revisit a step. The loop from that step is the cycle.
fn find_cycle(ids: &[String], deps: &[Vec<usize>], done: &[bool]) -> Vec<String> {
    let Some(start) = done.iter().position(|d| !*d) else {
        return Vec::new();
    };
    let mut path: Vec<usize> = Vec::new();
    let mut position: HashMap<usize, usize> = HashMap::new();
    let mut current = start;
    loop {
        if let Some(&p) = position.get(&current) {
            return path[p..].iter().map(|&i| ids[i].clone()).collect();
        }
        position.insert(current, path.len());
        path.push(current);
        match deps[current].iter().copied().find(|&d| !done[d]) {
            Some(next) => current = next,
            None => return path.iter().map(|&i| ids[i].clone()).collect(),
        }
    }
}

/// One execution of a [`Pipeline`], with one [`RunState`] per step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineRun<'a> {
    pipeline: &'a Pipeline,
    /// Indexed by step index, as in [`Pipeline::ids`].
    states: Vec<RunState>,
}

impl PipelineRun<'_> {
    /// Current state of a step.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::UnknownStep`] when the id is not in the pipeline.
    pub fn state(&self, id: &str) -> Result<&RunState, PipelineError> {
        let i = self.pipeline.position(id)?;
        Ok(&self.states[i])
    }

    /// Queued steps whose dependencies have all succeeded, in execution order.
    #[must_use]
    pub fn ready(&self) -> Vec<&str> {
        self.pipeline
            .order
            .iter()
            .copied()
            .filter(|&i| {
                matches!(self.states[i], RunState::Queued)
                    && self.pipeline.deps[i]
                        .iter()
                        .all(|&d| matches!(self.states[d], RunState::Succeeded { .. }))
            })
            .map(|i| self.pipeline.ids[i].as_str())
            .collect()
    }

    /// Queued steps that can never run because a dependency reached a terminal state
    /// other than success, in execution order.
    #[must_use]
    pub fn blocked(&self) -> Vec<&str> {
        self.pipeline
            .order
            .iter()
            .copied()
            .filter(|&i| {
                matches!(self.states[i], RunState::Queued)
                    && self.pipeline.deps[i].iter().any(|&d| {
                        self.states[d].is_terminal()
                            && !matches!(self.states[d], RunState::Succeeded { .. })
                    })
            })
            .map(|i| self.pipeline.ids[i].as_str())
            .collect()
    }

    /// True when every step has succeeded.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.states
            .iter()
            .all(|s| matches!(s, RunState::Succeeded { .. }))
    }

    /// Applies an event to one step, in memory only.
    ///
    /// Starting a step also requires every dependency to be `Succeeded`. Other
    /// events are checked by [`transition`].
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::UnknownStep`], [`PipelineError::DependencyNotSatisfied`],
    /// or [`PipelineError::Transition`] when the step's lifecycle refuses the event.
    /// The state is unchanged on error.
    pub fn apply(&mut self, id: &str, event: &Event) -> Result<(), PipelineError> {
        let i = self.pipeline.position(id)?;
        if *event == Event::Start {
            for &d in &self.pipeline.deps[i] {
                if !matches!(self.states[d], RunState::Succeeded { .. }) {
                    return Err(PipelineError::DependencyNotSatisfied {
                        step: id.to_owned(),
                        dependency: self.pipeline.ids[d].clone(),
                    });
                }
            }
        }
        let next = transition(&self.states[i], event).map_err(PipelineError::Transition)?;
        self.states[i] = next;
        Ok(())
    }
}

#[cfg(test)]
#[path = "pipeline_test.rs"]
mod pipeline_test;
