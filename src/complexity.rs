//! Cyclomatic and cognitive complexity for Luau functions.
//!
//! Formulas follow https://docs.fallow.tools/explanations/health
//! Cognitive model follows SonarSource's published cognitive-complexity rules
//! adapted to Luau control flow (`if`/`elseif`/`else`, loops, `and`/`or`,
//! Luau if-expressions). Every function body is a unit, including nested
//! `local function` and anonymous `function() ... end`.

use full_moon::ast::{
    Ast, BinOp, Block, Call, Expression, FunctionArgs, FunctionBody, Prefix, Stmt, Suffix, Var,
};
use full_moon::node::Node;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskBin {
    LowRisk,
    MediumRisk,
    HighRisk,
    VeryHighRisk,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RiskProfile {
    pub low_risk: f64,
    pub medium_risk: f64,
    pub high_risk: f64,
    pub very_high_risk: f64,
}

impl RiskProfile {
    pub fn from_counts(counts: [usize; 4], total: usize) -> Self {
        if total == 0 {
            return Self::default();
        }
        let t = total as f64;
        Self {
            low_risk: (counts[0] as f64) * 100.0 / t,
            medium_risk: (counts[1] as f64) * 100.0 / t,
            high_risk: (counts[2] as f64) * 100.0 / t,
            very_high_risk: (counts[3] as f64) * 100.0 / t,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionMetrics {
    pub name: String,
    pub line: usize,
    pub end_line: usize,
    pub lines: usize,
    pub parameters: usize,
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub unit_size_bin: RiskBin,
    pub unit_interfacing_bin: RiskBin,
}

/// Analyze all function units in a parsed AST (nested included).
pub fn analyze_functions(ast: &Ast) -> Vec<FunctionMetrics> {
    let mut out = Vec::new();
    let mut stack = Vec::new();
    collect_block(ast.nodes(), &mut stack, &mut out);
    out
}

fn qualify(stack: &[String], name: &str) -> String {
    if stack.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", stack.join("/"), name)
    }
}

fn record_unit(
    stack: &[String],
    name: &str,
    body: &FunctionBody,
    start_fallback: usize,
    out: &mut Vec<FunctionMetrics>,
) {
    let start = body
        .parameters_parentheses()
        .tokens()
        .0
        .start_position()
        .map(|p| p.line())
        .unwrap_or(start_fallback);
    let end = body
        .end_token()
        .end_position()
        .map(|p| p.line())
        .unwrap_or(start);
    let lines = end.saturating_sub(start).saturating_add(1);
    let parameters = body.parameters().len();

    let mut scorer = ComplexityScorer::new();
    scorer.score_block(body.block(), 0);

    out.push(FunctionMetrics {
        name: qualify(stack, name),
        line: start,
        end_line: end,
        lines,
        parameters,
        cyclomatic: 1 + scorer.cyclomatic_extra,
        cognitive: scorer.cognitive,
        unit_size_bin: unit_size_bin(lines),
        unit_interfacing_bin: unit_interfacing_bin(parameters),
    });
}

fn collect_block(block: &Block, stack: &mut Vec<String>, out: &mut Vec<FunctionMetrics>) {
    for stmt in block.stmts() {
        collect_stmt(stmt, stack, out);
    }
}

fn collect_stmt(stmt: &Stmt, stack: &mut Vec<String>, out: &mut Vec<FunctionMetrics>) {
    match stmt {
        Stmt::LocalFunction(func) => {
            let name = func.name().token().to_string();
            let start = func
                .function_token()
                .start_position()
                .map(|p| p.line())
                .unwrap_or(1);
            record_unit(stack, &name, func.body(), start, out);
            stack.push(name);
            collect_block(func.body().block(), stack, out);
            stack.pop();
        }
        Stmt::FunctionDeclaration(func) => {
            let name = func.name().to_string().replace(' ', "");
            let start = func
                .function_token()
                .start_position()
                .map(|p| p.line())
                .unwrap_or(1);
            record_unit(stack, &name, func.body(), start, out);
            stack.push(name);
            collect_block(func.body().block(), stack, out);
            stack.pop();
        }
        Stmt::ConstFunction(func) => {
            let name = func.name().token().to_string();
            let start = func
                .function_token()
                .start_position()
                .map(|p| p.line())
                .unwrap_or(1);
            record_unit(stack, &name, func.body(), start, out);
            stack.push(name);
            collect_block(func.body().block(), stack, out);
            stack.pop();
        }
        Stmt::Do(d) => collect_block(d.block(), stack, out),
        Stmt::If(if_stmt) => {
            collect_expression(if_stmt.condition(), stack, out);
            collect_block(if_stmt.block(), stack, out);
            for else_if in if_stmt.else_if().into_iter().flatten() {
                collect_expression(else_if.condition(), stack, out);
                collect_block(else_if.block(), stack, out);
            }
            if let Some(else_block) = if_stmt.else_block() {
                collect_block(else_block, stack, out);
            }
        }
        Stmt::While(w) => {
            collect_expression(w.condition(), stack, out);
            collect_block(w.block(), stack, out);
        }
        Stmt::Repeat(r) => {
            collect_block(r.block(), stack, out);
            collect_expression(r.until(), stack, out);
        }
        Stmt::NumericFor(f) => {
            collect_expression(f.start(), stack, out);
            collect_expression(f.end(), stack, out);
            if let Some(step) = f.step() {
                collect_expression(step, stack, out);
            }
            collect_block(f.block(), stack, out);
        }
        Stmt::GenericFor(f) => {
            for expr in f.expressions() {
                collect_expression(expr, stack, out);
            }
            collect_block(f.block(), stack, out);
        }
        Stmt::Assignment(a) => {
            for expr in a.expressions() {
                collect_expression(expr, stack, out);
            }
        }
        Stmt::LocalAssignment(a) => {
            for expr in a.expressions() {
                collect_expression(expr, stack, out);
            }
        }
        Stmt::ConstAssignment(a) => {
            for expr in a.expressions() {
                collect_expression(expr, stack, out);
            }
        }
        Stmt::FunctionCall(call) => collect_call(call, stack, out),
        Stmt::CompoundAssignment(c) => collect_expression(c.rhs(), stack, out),
        _ => {}
    }
}

fn collect_expression(expr: &Expression, stack: &mut Vec<String>, out: &mut Vec<FunctionMetrics>) {
    match expr {
        Expression::Function(anon) => {
            let start = anon
                .function_token()
                .start_position()
                .map(|p| p.line())
                .unwrap_or(1);
            record_unit(stack, "<anonymous>", anon.body(), start, out);
            stack.push("<anonymous>".into());
            collect_block(anon.body().block(), stack, out);
            stack.pop();
        }
        Expression::BinaryOperator { lhs, rhs, .. } => {
            collect_expression(lhs, stack, out);
            collect_expression(rhs, stack, out);
        }
        Expression::UnaryOperator { expression, .. } => {
            collect_expression(expression, stack, out)
        }
        Expression::Parentheses { expression, .. } => collect_expression(expression, stack, out),
        Expression::FunctionCall(call) => collect_call(call, stack, out),
        Expression::IfExpression(if_expr) => {
            collect_expression(if_expr.condition(), stack, out);
            collect_expression(if_expr.if_expression(), stack, out);
            for else_if in if_expr.else_if_expressions().into_iter().flatten() {
                collect_expression(else_if.condition(), stack, out);
                collect_expression(else_if.expression(), stack, out);
            }
            collect_expression(if_expr.else_expression(), stack, out);
        }
        Expression::TableConstructor(table) => {
            for field in table.fields() {
                match field {
                    full_moon::ast::Field::ExpressionKey { key, value, .. } => {
                        collect_expression(key, stack, out);
                        collect_expression(value, stack, out);
                    }
                    full_moon::ast::Field::NameKey { value, .. } => {
                        collect_expression(value, stack, out)
                    }
                    full_moon::ast::Field::NoKey(value) => collect_expression(value, stack, out),
                    _ => {}
                }
            }
        }
        Expression::TypeAssertion { expression, .. } => {
            collect_expression(expression, stack, out)
        }
        Expression::Var(var) => collect_var(var, stack, out),
        Expression::InterpolatedString(s) => {
            for e in s.expressions() {
                collect_expression(e, stack, out);
            }
        }
        _ => {}
    }
}

fn collect_call(
    call: &full_moon::ast::FunctionCall,
    stack: &mut Vec<String>,
    out: &mut Vec<FunctionMetrics>,
) {
    if let Prefix::Expression(expr) = call.prefix() {
        collect_expression(expr, stack, out);
    }
    for suffix in call.suffixes() {
        match suffix {
            Suffix::Call(Call::AnonymousCall(args)) => collect_args(args, stack, out),
            Suffix::Call(Call::MethodCall(m)) => collect_args(m.args(), stack, out),
            Suffix::Index(full_moon::ast::Index::Brackets { expression, .. }) => {
                collect_expression(expression, stack, out)
            }
            _ => {}
        }
    }
}

fn collect_args(args: &FunctionArgs, stack: &mut Vec<String>, out: &mut Vec<FunctionMetrics>) {
    match args {
        FunctionArgs::Parentheses { arguments, .. } => {
            for expr in arguments {
                collect_expression(expr, stack, out);
            }
        }
        FunctionArgs::TableConstructor(table) => {
            collect_expression(&Expression::TableConstructor(table.clone()), stack, out);
        }
        _ => {}
    }
}

fn collect_var(var: &Var, stack: &mut Vec<String>, out: &mut Vec<FunctionMetrics>) {
    match var {
        Var::Expression(ve) => {
            if let Prefix::Expression(expr) = ve.prefix() {
                collect_expression(expr, stack, out);
            }
            for suffix in ve.suffixes() {
                match suffix {
                    Suffix::Call(Call::AnonymousCall(args)) => collect_args(args, stack, out),
                    Suffix::Call(Call::MethodCall(m)) => collect_args(m.args(), stack, out),
                    Suffix::Index(full_moon::ast::Index::Brackets { expression, .. }) => {
                        collect_expression(expression, stack, out)
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// SIG unit-size bins (LOC).
pub fn unit_size_bin(lines: usize) -> RiskBin {
    match lines {
        0..=15 => RiskBin::LowRisk,
        16..=30 => RiskBin::MediumRisk,
        31..=60 => RiskBin::HighRisk,
        _ => RiskBin::VeryHighRisk,
    }
}

/// SIG unit-interfacing bins (parameter count).
pub fn unit_interfacing_bin(params: usize) -> RiskBin {
    match params {
        0..=2 => RiskBin::LowRisk,
        3..=4 => RiskBin::MediumRisk,
        5..=6 => RiskBin::HighRisk,
        _ => RiskBin::VeryHighRisk,
    }
}

struct ComplexityScorer {
    cyclomatic_extra: u32,
    cognitive: u32,
}

impl ComplexityScorer {
    fn new() -> Self {
        Self {
            cyclomatic_extra: 0,
            cognitive: 0,
        }
    }

    fn score_block(&mut self, block: &Block, nesting: u32) {
        for stmt in block.stmts() {
            self.score_stmt(stmt, nesting);
        }
    }

    fn score_stmt(&mut self, stmt: &Stmt, nesting: u32) {
        match stmt {
            Stmt::If(if_stmt) => {
                self.cyclomatic_extra += 1;
                self.cognitive += 1 + nesting;
                self.score_expression(if_stmt.condition(), nesting);
                self.score_block(if_stmt.block(), nesting + 1);
                for else_if in if_stmt.else_if().into_iter().flatten() {
                    self.cyclomatic_extra += 1;
                    self.cognitive += 1 + nesting;
                    self.score_expression(else_if.condition(), nesting);
                    self.score_block(else_if.block(), nesting + 1);
                }
                if let Some(else_block) = if_stmt.else_block() {
                    self.cognitive += 1;
                    self.score_block(else_block, nesting + 1);
                }
            }
            Stmt::While(w) => {
                self.cyclomatic_extra += 1;
                self.cognitive += 1 + nesting;
                self.score_expression(w.condition(), nesting);
                self.score_block(w.block(), nesting + 1);
            }
            Stmt::Repeat(r) => {
                self.cyclomatic_extra += 1;
                self.cognitive += 1 + nesting;
                self.score_block(r.block(), nesting + 1);
                self.score_expression(r.until(), nesting);
            }
            Stmt::NumericFor(f) => {
                self.cyclomatic_extra += 1;
                self.cognitive += 1 + nesting;
                self.score_expression(f.start(), nesting);
                self.score_expression(f.end(), nesting);
                if let Some(step) = f.step() {
                    self.score_expression(step, nesting);
                }
                self.score_block(f.block(), nesting + 1);
            }
            Stmt::GenericFor(f) => {
                self.cyclomatic_extra += 1;
                self.cognitive += 1 + nesting;
                for expr in f.expressions() {
                    self.score_expression(expr, nesting);
                }
                self.score_block(f.block(), nesting + 1);
            }
            Stmt::Do(d) => self.score_block(d.block(), nesting),
            Stmt::Assignment(a) => {
                for expr in a.expressions() {
                    self.score_expression(expr, nesting);
                }
            }
            Stmt::LocalAssignment(a) => {
                for expr in a.expressions() {
                    self.score_expression(expr, nesting);
                }
            }
            Stmt::FunctionCall(call) => self.score_call(call, nesting),
            Stmt::CompoundAssignment(c) => self.score_expression(c.rhs(), nesting),
            Stmt::LocalFunction(_) | Stmt::FunctionDeclaration(_) | Stmt::ConstFunction(_) => {}
            Stmt::ConstAssignment(a) => {
                for expr in a.expressions() {
                    self.score_expression(expr, nesting);
                }
            }
            _ => {}
        }
    }

    fn score_expression(&mut self, expr: &Expression, nesting: u32) {
        match expr {
            Expression::BinaryOperator { lhs, binop, rhs } => {
                if is_logical(binop) {
                    self.cyclomatic_extra += 1;
                    self.cognitive += 1;
                }
                self.score_expression(lhs, nesting);
                self.score_expression(rhs, nesting);
            }
            Expression::UnaryOperator { expression, .. } => {
                self.score_expression(expression, nesting)
            }
            Expression::Parentheses { expression, .. } => {
                self.score_expression(expression, nesting)
            }
            Expression::FunctionCall(call) => self.score_call(call, nesting),
            Expression::IfExpression(if_expr) => {
                self.cyclomatic_extra += 1;
                self.cognitive += 1 + nesting;
                self.score_expression(if_expr.condition(), nesting);
                self.score_expression(if_expr.if_expression(), nesting + 1);
                for else_if in if_expr.else_if_expressions().into_iter().flatten() {
                    self.cyclomatic_extra += 1;
                    self.cognitive += 1 + nesting;
                    self.score_expression(else_if.condition(), nesting);
                    self.score_expression(else_if.expression(), nesting + 1);
                }
                self.score_expression(if_expr.else_expression(), nesting + 1);
            }
            Expression::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        full_moon::ast::Field::ExpressionKey { key, value, .. } => {
                            self.score_expression(key, nesting);
                            self.score_expression(value, nesting);
                        }
                        full_moon::ast::Field::NameKey { value, .. } => {
                            self.score_expression(value, nesting)
                        }
                        full_moon::ast::Field::NoKey(value) => {
                            self.score_expression(value, nesting)
                        }
                        _ => {}
                    }
                }
            }
            Expression::TypeAssertion { expression, .. } => {
                self.score_expression(expression, nesting)
            }
            Expression::Var(var) => self.score_var(var, nesting),
            Expression::Function(_) => {}
            Expression::InterpolatedString(s) => {
                for e in s.expressions() {
                    self.score_expression(e, nesting);
                }
            }
            _ => {}
        }
    }

    fn score_call(&mut self, call: &full_moon::ast::FunctionCall, nesting: u32) {
        if let Prefix::Expression(expr) = call.prefix() {
            self.score_expression(expr, nesting);
        }
        for suffix in call.suffixes() {
            match suffix {
                Suffix::Call(Call::AnonymousCall(args)) => self.score_args(args, nesting),
                Suffix::Call(Call::MethodCall(m)) => self.score_args(m.args(), nesting),
                Suffix::Index(full_moon::ast::Index::Brackets { expression, .. }) => {
                    self.score_expression(expression, nesting)
                }
                _ => {}
            }
        }
    }

    fn score_args(&mut self, args: &FunctionArgs, nesting: u32) {
        match args {
            FunctionArgs::Parentheses { arguments, .. } => {
                for expr in arguments {
                    self.score_expression(expr, nesting);
                }
            }
            FunctionArgs::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        full_moon::ast::Field::ExpressionKey { key, value, .. } => {
                            self.score_expression(key, nesting);
                            self.score_expression(value, nesting);
                        }
                        full_moon::ast::Field::NameKey { value, .. } => {
                            self.score_expression(value, nesting)
                        }
                        full_moon::ast::Field::NoKey(value) => {
                            self.score_expression(value, nesting)
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn score_var(&mut self, var: &Var, nesting: u32) {
        match var {
            Var::Expression(ve) => {
                if let Prefix::Expression(expr) = ve.prefix() {
                    self.score_expression(expr, nesting);
                }
                for suffix in ve.suffixes() {
                    match suffix {
                        Suffix::Call(Call::AnonymousCall(args)) => self.score_args(args, nesting),
                        Suffix::Call(Call::MethodCall(m)) => self.score_args(m.args(), nesting),
                        Suffix::Index(full_moon::ast::Index::Brackets { expression, .. }) => {
                            self.score_expression(expression, nesting)
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn is_logical(op: &BinOp) -> bool {
    matches!(op, BinOp::And(_) | BinOp::Or(_))
}

pub fn profiles_from_functions(functions: &[FunctionMetrics]) -> (RiskProfile, RiskProfile) {
    let mut size = [0usize; 4];
    let mut iface = [0usize; 4];
    for f in functions {
        size[bin_index(f.unit_size_bin)] += 1;
        iface[bin_index(f.unit_interfacing_bin)] += 1;
    }
    let n = functions.len();
    (
        RiskProfile::from_counts(size, n),
        RiskProfile::from_counts(iface, n),
    )
}

fn bin_index(bin: RiskBin) -> usize {
    match bin {
        RiskBin::LowRisk => 0,
        RiskBin::MediumRisk => 1,
        RiskBin::HighRisk => 2,
        RiskBin::VeryHighRisk => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use full_moon::parse;

    #[test]
    fn nested_local_function_is_its_own_unit() {
        let src = r#"
local function outer()
    local function inner(a, b)
        if a then
            if b then
                return 1
            end
        end
        return 0
    end
    return inner
end
"#;
        let ast = parse(src).unwrap();
        let fns = analyze_functions(&ast);
        assert!(
            fns.iter().any(|f| f.name == "outer"),
            "missing outer: {fns:?}"
        );
        assert!(
            fns.iter().any(|f| f.name.contains("inner")),
            "missing inner: {fns:?}"
        );
        let inner = fns.iter().find(|f| f.name.contains("inner")).unwrap();
        assert!(inner.cyclomatic >= 3, "inner cc={}", inner.cyclomatic);
        assert!(inner.cognitive >= 3, "inner cog={}", inner.cognitive);
    }

    #[test]
    fn flat_if_elseif_chain() {
        let src = r#"
local function classify(x)
    if x == 1 then
        return "a"
    elseif x == 2 then
        return "b"
    elseif x == 3 then
        return "c"
    else
        return "d"
    end
end
"#;
        let ast = parse(src).unwrap();
        let fns = analyze_functions(&ast);
        let f = &fns[0];
        assert_eq!(f.cyclomatic, 4);
        assert!(f.cognitive >= 4);
    }
}
