//! A delegation plan: steps that name the steps they depend on.
//!
//! A plan is a directed acyclic graph. [`Plan::order`] checks the ids and
//! dependencies, rejects cycles, and returns the steps in an order where every
//! step comes after its dependencies. Ties go to the step declared first.
//! [`run`] walks that order and hands each step to a caller-supplied executor,
//! so the real providers are reached only through the closure. Tests pass a
//! fake executor and never call a provider.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{run_chain, ChainOptions, Request};
use crate::config::delegate::DelegateConfig;

/// One delegated prompt inside a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Unique name within the plan. Dependencies refer to it.
    pub id: String,
    /// The bounded prompt sent for this step.
    pub prompt: String,
    /// Chain name from `delegate.chains`. Empty means the executor decides.
    #[serde(default)]
    pub chain: String,
    /// Ids of the steps whose output this step needs.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// An ordered list of steps with dependencies between them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub steps: Vec<Step>,
}

/// A plan that cannot be run as written.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlanError {
    #[error("a step has an empty id")]
    EmptyId,
    #[error("step id {0:?} is used more than once")]
    DuplicateId(String),
    #[error("step {step:?} depends on unknown step {dep:?}")]
    UnknownDependency { step: String, dep: String },
    /// The loop is listed in dependency order, and the first id is repeated at the end.
    #[error("dependency cycle: {}", .0.join(" -> "))]
    Cycle(Vec<String>),
}

/// Why a run stopped.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RunError {
    #[error(transparent)]
    Plan(#[from] PlanError),
    /// The executor failed on this step. Later steps were not run.
    #[error("step {id:?} failed: {message}")]
    Step { id: String, message: String },
    /// A step names a chain that is not in `delegate.chains`. Found before any call is made.
    #[error("step {id:?} names unknown chain {chain:?}")]
    UnknownChain { id: String, chain: String },
}

/// The output of one finished step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRun {
    pub id: String,
    pub text: String,
}

impl Plan {
    /// Checks the plan and returns the steps in a valid execution order.
    ///
    /// # Errors
    ///
    /// [`PlanError`] for an empty or duplicate id, an unknown dependency, or a cycle.
    pub fn order(&self) -> Result<Vec<&Step>, PlanError> {
        let index = self.index()?;
        let n = self.steps.len();
        let mut indegree = vec![0usize; n];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, step) in self.steps.iter().enumerate() {
            for dep in &step.depends_on {
                let j = index[dep.as_str()];
                indegree[i] += 1;
                dependents[j].push(i);
            }
        }

        // Kahn's algorithm. The heap yields the smallest declaration index first,
        // which keeps the order stable for the same input.
        let mut ready: BinaryHeap<Reverse<usize>> =
            (0..n).filter(|&i| indegree[i] == 0).map(Reverse).collect();
        let mut order: Vec<&Step> = Vec::with_capacity(n);
        while let Some(Reverse(i)) = ready.pop() {
            order.push(&self.steps[i]);
            for &k in &dependents[i] {
                indegree[k] -= 1;
                if indegree[k] == 0 {
                    ready.push(Reverse(k));
                }
            }
        }

        if order.len() < n {
            return Err(PlanError::Cycle(self.find_cycle(&index, &indegree)));
        }
        Ok(order)
    }

    /// Checks the plan without returning the order.
    ///
    /// # Errors
    ///
    /// The same errors as [`Plan::order`].
    pub fn validate(&self) -> Result<(), PlanError> {
        self.order().map(|_| ())
    }

    /// Maps each id to its position in `steps`. Also checks ids and dependencies.
    fn index(&self) -> Result<BTreeMap<&str, usize>, PlanError> {
        let mut index: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, step) in self.steps.iter().enumerate() {
            if step.id.trim().is_empty() {
                return Err(PlanError::EmptyId);
            }
            if index.insert(step.id.as_str(), i).is_some() {
                return Err(PlanError::DuplicateId(step.id.clone()));
            }
        }
        for step in &self.steps {
            for dep in &step.depends_on {
                if !index.contains_key(dep.as_str()) {
                    return Err(PlanError::UnknownDependency {
                        step: step.id.clone(),
                        dep: dep.clone(),
                    });
                }
            }
        }
        Ok(index)
    }

    /// Finds one cycle among the steps Kahn's algorithm could not place.
    /// `indegree` is the state after that pass: a step still above zero was never placed.
    /// Steps that only depend on a cycle are left out of the result.
    fn find_cycle(&self, index: &BTreeMap<&str, usize>, indegree: &[usize]) -> Vec<String> {
        let Some(start) = indegree.iter().position(|&d| d > 0) else {
            return Vec::new();
        };
        let mut path: Vec<usize> = Vec::new();
        let mut seen: Vec<Option<usize>> = vec![None; self.steps.len()];
        let mut cur = start;
        loop {
            if let Some(at) = seen[cur] {
                let mut cycle: Vec<String> = path
                    .get(at..)
                    .unwrap_or_default()
                    .iter()
                    .map(|&i| self.steps[i].id.clone())
                    .collect();
                cycle.push(self.steps[cur].id.clone());
                return cycle;
            }
            seen[cur] = Some(path.len());
            path.push(cur);
            // An unplaced step always has an unplaced dependency, so this finds one.
            let next = self.steps[cur]
                .depends_on
                .iter()
                .filter_map(|d| index.get(d.as_str()).copied())
                .find(|&j| indegree[j] > 0);
            match next {
                Some(j) => cur = j,
                None => return path.iter().map(|&i| self.steps[i].id.clone()).collect(),
            }
        }
    }
}

/// Runs the plan in order. `exec` gets each step and the outputs of that step's
/// dependencies, keyed by id. The run stops at the first failing step.
///
/// # Errors
///
/// [`RunError::Plan`] if the plan is invalid, before any step runs. [`RunError::Step`]
/// for the first step whose executor fails.
pub fn run<F>(plan: &Plan, mut exec: F) -> Result<Vec<StepRun>, RunError>
where
    F: FnMut(&Step, &BTreeMap<String, String>) -> Result<String, String>,
{
    let order = plan.order()?;
    let mut outputs: BTreeMap<String, String> = BTreeMap::new();
    let mut runs: Vec<StepRun> = Vec::with_capacity(order.len());
    for step in order {
        let inputs: BTreeMap<String, String> = step
            .depends_on
            .iter()
            .filter_map(|d| outputs.get(d).map(|text| (d.clone(), text.clone())))
            .collect();
        let text = exec(step, &inputs).map_err(|message| RunError::Step {
            id: step.id.clone(),
            message,
        })?;
        outputs.insert(step.id.clone(), text.clone());
        runs.push(StepRun {
            id: step.id.clone(),
            text,
        });
    }
    Ok(runs)
}

/// Runs the plan through the provider chains in `cfg`, using the existing
/// [`run_chain`] path, so fall-through, rate limits, budget checks and usage rules all apply.
///
/// A step with an empty `chain` uses `default_chain`. Every step's chain is resolved
/// before the first call, so an unknown name costs no tokens. `base` supplies `timeout`,
/// `max_output_tokens` and `dir` for each request. `opts` supplies the confirm flag, retry
/// limit, provider builder, check and sleep. Its `chain` field is ignored, because each step
/// has its own chain.
///
/// # Errors
///
/// [`RunError::Plan`] for an invalid plan, [`RunError::UnknownChain`] before any call,
/// and [`RunError::Step`] for the first step no provider in its chain answers.
pub fn run_with_chains(
    plan: &Plan,
    cfg: &DelegateConfig,
    default_chain: &str,
    base: &Request,
    opts: &ChainOptions<'_>,
) -> Result<Vec<StepRun>, RunError> {
    let mut resolved: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    for step in &plan.steps {
        let chain = if step.chain.is_empty() {
            default_chain
        } else {
            step.chain.as_str()
        };
        let names = cfg
            .chains
            .get(chain)
            .ok_or_else(|| RunError::UnknownChain {
                id: step.id.clone(),
                chain: chain.to_owned(),
            })?;
        resolved.insert(step.id.clone(), (chain.to_owned(), names.clone()));
    }

    run(plan, |step: &Step, inputs: &BTreeMap<String, String>| {
        let Some((chain, names)) = resolved.get(&step.id) else {
            return Err(format!("no chain was resolved for step {:?}", step.id));
        };
        let req = Request {
            prompt: build_prompt(step, inputs),
            max_output_tokens: base.max_output_tokens,
            timeout: base.timeout,
            dir: base.dir.clone(),
        };
        let step_opts = ChainOptions {
            chain: chain.clone(),
            confirm: opts.confirm,
            max_retry_after: opts.max_retry_after,
            build: opts.build,
            check: opts.check,
            sleep: opts.sleep,
        };
        match run_chain(cfg, names, &req, &step_opts) {
            Ok((response, _attempts)) => Ok(response.text),
            Err(e) => Err(e.to_string()),
        }
    })
}

/// The step prompt followed by the output of each dependency, in id order.
fn build_prompt(step: &Step, inputs: &BTreeMap<String, String>) -> String {
    let mut prompt = step.prompt.clone();
    for (id, text) in inputs {
        prompt.push_str(&format!("\n\n--- output of step {id} ---\n{text}"));
    }
    prompt
}

#[cfg(test)]
#[path = "plan_test.rs"]
mod plan_test;
