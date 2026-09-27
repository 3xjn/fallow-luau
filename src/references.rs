//! Literal imported-key references with lexical local/parameter shadowing.
use crate::graph::{display_rel, unquote, RequireGraph};
use full_moon::{ast::*, node::Node, visitors::Visitor};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Reference {
    pub from: String,
    pub to: String,
    /// None means a whole-module escape or computed key, not a known symbol.
    pub key: Option<String>,
    pub line: usize,
}

pub(crate) fn collect(
    root: &Path,
    files: &[PathBuf],
    graph: &RequireGraph,
) -> Result<Vec<Reference>, String> {
    let mut result = Vec::new();
    for file in files {
        let from = display_rel(root, file);
        let source = std::fs::read_to_string(file).map_err(|e| format!("read {from}: {e}"))?;
        let ast = full_moon::parse(&source).map_err(|e| format!("parse {from}: {e:?}"))?;
        let edges = graph
            .edges
            .iter()
            .filter(|e| e.from == from)
            .filter_map(|e| Some((e.specifier.clone()?, e.to.clone()?)))
            .collect();
        let mut visitor = References {
            from,
            edges,
            scopes: vec![BTreeMap::new()],
            found: Vec::new(),
        };
        visitor.visit_ast(&ast);
        result.extend(visitor.found);
    }
    result.sort_by(|a, b| (&a.from, a.line, &a.to, &a.key).cmp(&(&b.from, b.line, &b.to, &b.key)));
    result.dedup_by(|a, b| a.from == b.from && a.to == b.to && a.line == b.line && a.key == b.key);
    Ok(result)
}

struct References {
    from: String,
    edges: BTreeMap<String, String>,
    scopes: Vec<BTreeMap<String, Option<String>>>,
    found: Vec<Reference>,
}

impl References {
    fn binding(&self, name: &str) -> Option<String> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.get(name))
            .cloned()
            .flatten()
    }
    fn require(&self, prefix: &Prefix, args: &FunctionArgs) -> Option<String> {
        let Prefix::Name(name) = prefix else {
            return None;
        };
        if name.token().to_string() != "require"
            || self.scopes.iter().any(|s| s.contains_key("require"))
        {
            return None;
        }
        let token = match args {
            FunctionArgs::String(t) => t,
            FunctionArgs::Parentheses { arguments, .. } => match arguments.iter().next()? {
                Expression::String(t) => t,
                _ => return None,
            },
            _ => return None,
        };
        self.edges
            .get(&unquote(&token.token().to_string()))
            .cloned()
    }
    fn module(&self, expr: &Expression) -> Option<String> {
        match expr {
            Expression::Var(Var::Name(n)) => self.binding(&n.token().to_string()),
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => self.module(expression),
            Expression::FunctionCall(c) => {
                let mut suffixes = c.suffixes();
                let Suffix::Call(Call::AnonymousCall(args)) = suffixes.next()? else {
                    return None;
                };
                if suffixes.next().is_some() {
                    return None;
                }
                self.require(c.prefix(), args)
            }
            _ => None,
        }
    }
    fn record(&mut self, to: String, key: Option<String>, line: usize) {
        self.found.push(Reference {
            from: self.from.clone(),
            to,
            key,
            line,
        });
    }
    fn access<'a>(
        &mut self,
        prefix: &Prefix,
        mut suffixes: impl Iterator<Item = &'a Suffix>,
        line: usize,
    ) {
        let mut first = suffixes.next();
        let mut module = match prefix {
            Prefix::Name(n) => self.binding(&n.token().to_string()),
            Prefix::Expression(e) => self.module(e),
            _ => None,
        };
        if module.is_none() {
            if let Some(Suffix::Call(Call::AnonymousCall(args))) = first {
                module = self.require(prefix, args);
                first = suffixes.next();
            }
        }
        let Some(to) = module else { return };
        let key = match first {
            Some(Suffix::Index(Index::Dot { name, .. })) => Some(name.token().to_string()),
            Some(Suffix::Index(Index::Brackets { expression, .. })) => match expression {
                Expression::String(t) => Some(unquote(&t.token().to_string())),
                _ => None,
            },
            Some(Suffix::Call(Call::MethodCall(m))) => Some(m.name().token().to_string()),
            // A bare require is a binding initializer, not an escape by itself.
            None => return,
            _ => None,
        };
        self.record(to, key, line);
    }
}

impl Visitor for References {
    fn visit_block(&mut self, _: &Block) {
        self.scopes.push(BTreeMap::new());
    }
    fn visit_block_end(&mut self, _: &Block) {
        self.scopes.pop();
    }
    fn visit_function_body(&mut self, body: &FunctionBody) {
        let mut scope = BTreeMap::new();
        for p in body.parameters() {
            if let Parameter::Name(n) = p {
                scope.insert(n.token().to_string(), None);
            }
        }
        self.scopes.push(scope);
    }
    fn visit_function_body_end(&mut self, _: &FunctionBody) {
        self.scopes.pop();
    }
    fn visit_local_assignment_end(&mut self, a: &LocalAssignment) {
        // RHS uses the outer binding (local m = m), then new names take effect.
        let values: Vec<_> = a
            .names()
            .iter()
            .enumerate()
            .map(|(i, n)| {
                (
                    n.token().to_string(),
                    a.expressions().iter().nth(i).and_then(|e| self.module(e)),
                )
            })
            .collect();
        self.scopes.last_mut().unwrap().extend(values);
    }
    fn visit_local_function(&mut self, f: &LocalFunction) {
        self.scopes
            .last_mut()
            .unwrap()
            .insert(f.name().token().to_string(), None);
    }
    fn visit_assignment_end(&mut self, a: &Assignment) {
        // Reassignment invalidates certainty. Do not invent control-flow reachability.
        for var in a.variables() {
            if let Var::Name(n) = var {
                let name = n.token().to_string();
                if let Some(to) = self.binding(&name) {
                    self.record(to, None, n.start_position().unwrap().line());
                }
                for scope in self.scopes.iter_mut().rev() {
                    if scope.contains_key(&name) {
                        scope.insert(name, None);
                        break;
                    }
                }
            }
        }
    }
    fn visit_numeric_for(&mut self, f: &NumericFor) {
        self.scopes.push(BTreeMap::from([(
            f.index_variable().token().to_string(),
            None,
        )]));
    }
    fn visit_numeric_for_end(&mut self, _: &NumericFor) {
        self.scopes.pop();
    }
    fn visit_generic_for(&mut self, f: &GenericFor) {
        self.scopes.push(
            f.names()
                .iter()
                .map(|n| (n.token().to_string(), None))
                .collect(),
        );
    }
    fn visit_generic_for_end(&mut self, _: &GenericFor) {
        self.scopes.pop();
    }
    fn visit_function_call(&mut self, c: &FunctionCall) {
        self.access(c.prefix(), c.suffixes(), c.start_position().unwrap().line());
    }
    fn visit_var_expression(&mut self, v: &VarExpression) {
        self.access(v.prefix(), v.suffixes(), v.start_position().unwrap().line());
    }
    fn visit_expression(&mut self, e: &Expression) {
        if let Expression::Var(Var::Name(n)) = e {
            if let Some(to) = self.binding(&n.token().to_string()) {
                self.record(to, None, n.start_position().unwrap().line());
            }
        }
    }
    fn visit_return(&mut self, r: &Return) {
        for e in r.returns() {
            if let Some(to) = self.module(e) {
                self.record(to, None, e.start_position().unwrap().line());
            }
        }
    }
    fn visit_function_args(&mut self, args: &FunctionArgs) {
        if let FunctionArgs::Parentheses { arguments, .. } = args {
            for e in arguments {
                if let Some(to) = self.module(e) {
                    self.record(to, None, e.start_position().unwrap().line());
                }
            }
        }
    }
}
