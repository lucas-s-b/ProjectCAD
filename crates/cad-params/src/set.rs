use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::lexer::{Tok, lex};
use crate::parser::{Expr, Func, parse};
use crate::value::{ParamError, Value, unit};

/// Named expressions in display order. Names are checked on `set`, so deserialized data may hold invalid entries; they evaluate to their error.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParamSet {
    entries: Vec<(String, String)>,
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && unit(name).is_none()
        && Func::from_name(name).is_none()
        && name != "pi"
        && name != "e"
}

/// One parsed entry with its direct dependencies on other entries (indices, by name order).
struct Node {
    expr: Result<Expr, ParamError>,
    vars: BTreeSet<String>,
    deps: Vec<usize>,
}

impl ParamSet {
    pub fn set(&mut self, name: &str, src: &str) -> Result<(), ParamError> {
        if !valid_name(name) {
            return Err(ParamError::InvalidName(name.to_string()));
        }
        parse(src)?;
        match self.entries.iter_mut().find(|(n, _)| n == name) {
            Some(entry) => entry.1 = src.to_string(),
            None => self.entries.push((name.to_string(), src.to_string())),
        }
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(n, _)| n != name);
        self.entries.len() != before
    }

    pub fn rename(&mut self, old: &str, new: &str) -> Result<(), ParamError> {
        if !valid_name(new) || self.get(new).is_some() {
            return Err(ParamError::InvalidName(new.to_string()));
        }
        if self.get(old).is_none() {
            return Err(ParamError::UnknownName(old.to_string()));
        }
        for (name, src) in &mut self.entries {
            if name == old {
                *name = new.to_string();
            }
            *src = rewrite_ident(src, old, new);
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries.iter().find(|(n, _)| n == name).map(|(_, s)| s.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(n, s)| (n.as_str(), s.as_str()))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn nodes(&self) -> Vec<Node> {
        let index: BTreeMap<&str, usize> = self.entries.iter().enumerate().rev().map(|(i, (n, _))| (n.as_str(), i)).collect();
        self.entries
            .iter()
            .map(|(_, src)| {
                let expr = parse(src);
                let vars = expr.as_ref().map(Expr::variables).unwrap_or_default();
                let deps = vars.iter().filter_map(|v| index.get(v.as_str()).copied()).collect();
                Node { expr, vars, deps }
            })
            .collect()
    }

    /// Kahn's algorithm, ties in display order. Returns the order and the indices left over (on or downstream of a cycle).
    fn sort(nodes: &[Node]) -> (Vec<usize>, Vec<usize>) {
        let mut indegree: Vec<usize> = nodes.iter().map(|n| n.deps.len()).collect();
        let mut users = vec![Vec::new(); nodes.len()];
        for (i, n) in nodes.iter().enumerate() {
            for &d in &n.deps {
                users[d].push(i);
            }
        }
        let mut ready: BTreeSet<usize> = (0..nodes.len()).filter(|&i| indegree[i] == 0).collect();
        let mut order = Vec::with_capacity(nodes.len());
        while let Some(i) = ready.pop_first() {
            order.push(i);
            for &u in &users[i] {
                indegree[u] -= 1;
                if indegree[u] == 0 {
                    ready.insert(u);
                }
            }
        }
        let rest = (0..nodes.len()).filter(|&i| indegree[i] > 0).collect();
        (order, rest)
    }

    /// A cycle through `start` as indices ending back at `start`, if one exists among `allowed`.
    fn cycle_through(nodes: &[Node], start: usize, allowed: &[bool]) -> Option<Vec<usize>> {
        let mut parent: BTreeMap<usize, usize> = BTreeMap::new();
        let mut queue = VecDeque::from([start]);
        while let Some(i) = queue.pop_front() {
            for &d in nodes[i].deps.iter().filter(|&&d| allowed[d]) {
                if d == start {
                    let mut path = vec![i];
                    while *path.last().expect("non-empty") != start {
                        path.push(parent[path.last().expect("non-empty")]);
                    }
                    path.reverse();
                    path.push(start);
                    return Some(path);
                }
                if let std::collections::btree_map::Entry::Vacant(e) = parent.entry(d) {
                    e.insert(i);
                    queue.push_back(d);
                }
            }
        }
        None
    }

    fn names(&self, idx: &[usize]) -> Vec<String> {
        idx.iter().map(|&i| self.entries[i].0.clone()).collect()
    }

    pub fn evaluate(&self) -> BTreeMap<String, Result<Value, ParamError>> {
        let nodes = self.nodes();
        let (order, rest) = Self::sort(&nodes);
        let mut allowed = vec![false; nodes.len()];
        rest.iter().for_each(|&i| allowed[i] = true);
        let mut results: Vec<Option<Result<Value, ParamError>>> = vec![None; nodes.len()];
        let mut failed = allowed.clone();
        for &i in &order {
            let r = self.eval_node(i, &nodes[i], &results, &failed);
            failed[i] = r.is_err();
            results[i] = Some(r);
        }
        for &i in &rest {
            let r = match Self::cycle_through(&nodes, i, &allowed) {
                Some(path) => ParamError::Cycle(self.names(&path)),
                None => first_failed(&self.entries, &nodes[i], &failed),
            };
            results[i] = Some(Err(r));
        }
        let mut out = BTreeMap::new();
        for (i, (name, _)) in self.entries.iter().enumerate() {
            out.insert(name.clone(), results[i].take().expect("every node has a result"));
        }
        out
    }

    fn eval_node(
        &self,
        i: usize,
        node: &Node,
        results: &[Option<Result<Value, ParamError>>],
        failed: &[bool],
    ) -> Result<Value, ParamError> {
        let name = &self.entries[i].0;
        if !valid_name(name) {
            return Err(ParamError::InvalidName(name.clone()));
        }
        let expr = node.expr.as_ref().map_err(Clone::clone)?;
        let known: BTreeSet<&str> = node.deps.iter().map(|&d| self.entries[d].0.as_str()).collect();
        if let Some(missing) = node.vars.iter().find(|v| !known.contains(v.as_str())) {
            return Err(ParamError::UnknownName(missing.clone()));
        }
        if node.deps.iter().any(|&d| failed[d]) {
            return Err(first_failed(&self.entries, node, failed));
        }
        let lookup = |n: &str| {
            let d = *node.deps.iter().find(|&&d| self.entries[d].0 == n)?;
            results[d].as_ref()?.as_ref().ok().copied()
        };
        expr.eval(&lookup)
    }

    pub fn eval_expr(&self, src: &str) -> Result<Value, ParamError> {
        let expr = parse(src)?;
        let values = self.evaluate();
        for v in expr.variables() {
            if matches!(values.get(&v), Some(Err(_))) {
                return Err(ParamError::Depends(v));
            }
        }
        expr.eval(&|n| values.get(n).and_then(|r| r.as_ref().ok()).copied())
    }

    pub fn dependencies(&self, name: &str) -> BTreeSet<String> {
        self.get(name).and_then(|src| parse(src).ok()).map(|e| e.variables()).unwrap_or_default()
    }

    pub fn dependents(&self, name: &str) -> BTreeSet<String> {
        let deps: Vec<(&String, BTreeSet<String>)> = self.entries.iter().map(|(n, _)| (n, self.dependencies(n))).collect();
        let mut found = BTreeSet::new();
        let mut queue = VecDeque::from([name.to_string()]);
        while let Some(cur) = queue.pop_front() {
            for (n, d) in &deps {
                if !found.contains(*n) && d.contains(&cur) {
                    found.insert((*n).clone());
                    queue.push_back((*n).clone());
                }
            }
        }
        found
    }

    pub fn order(&self) -> Result<Vec<String>, ParamError> {
        let nodes = self.nodes();
        let (order, rest) = Self::sort(&nodes);
        let Some(&first) = rest.first() else {
            return Ok(self.names(&order));
        };
        // Leftovers are on a cycle or downstream of one; skip the downstream ones.
        let mut allowed = vec![false; nodes.len()];
        rest.iter().for_each(|&i| allowed[i] = true);
        let on_cycle = rest.iter().find_map(|&i| Self::cycle_through(&nodes, i, &allowed)).unwrap_or_else(|| vec![first]);
        Err(ParamError::Cycle(self.names(&on_cycle)))
    }
}

/// The first direct dependency (in name order) that failed.
fn first_failed(entries: &[(String, String)], node: &Node, failed: &[bool]) -> ParamError {
    let name = node
        .vars
        .iter()
        .find(|v| node.deps.iter().any(|&d| failed[d] && entries[d].0 == v.as_str()))
        .cloned()
        .unwrap_or_default();
    ParamError::Depends(name)
}

/// Replaces whole identifier tokens, leaving the rest of the text untouched.
fn rewrite_ident(src: &str, old: &str, new: &str) -> String {
    let Ok(toks) = lex(src) else { return src.to_string() };
    let mut out = String::with_capacity(src.len());
    let mut last = 0;
    for (tok, pos) in toks {
        if matches!(&tok, Tok::Ident(n) if n == old) {
            out.push_str(&src[last..pos]);
            out.push_str(new);
            last = pos + old.len();
        }
    }
    out.push_str(&src[last..]);
    out
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod tests;
