use core::panic;
use std::collections::HashMap;

use crate::builtins::BUILTINTS;
use crate::parser::{Literal, Node, NumericLiteral, Operator, AST};
use crate::runtime_value::RuntimeValue;

struct Scope {
    // could make this a list of hashmaps that's search from the top down
    // would negate the need to duplicate the scope when adding items
    bindings: HashMap<String, RuntimeValue>,
}

impl Scope {
    fn new() -> Scope {
        Scope {
            bindings: HashMap::from_iter(
                BUILTINTS.map(|builtin| (builtin.name.to_string(), RuntimeValue::BuiltIn(builtin))),
            ),
        }
    }

    fn with_bindings(&self, bindings: Vec<(String, RuntimeValue)>) -> Scope {
        let mut new_bindings = self.bindings.clone();
        new_bindings.extend(bindings);

        Scope {
            bindings: new_bindings,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub params: Vec<String>,
    pub body: Node,
}

impl Function {
    fn eval(&self, args: &[RuntimeValue], scope: &Scope) -> RuntimeValue {
        if self.params.len() != args.len() {
            panic!("Function called with incorrect number of arguments");
        }

        // zip the args and params together
        let bindings = self
            .params
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect::<Vec<(String, RuntimeValue)>>();

        self.body.eval(&scope.with_bindings(bindings))
    }
}

impl RuntimeValue {
    fn from_literal(lit: &Literal) -> RuntimeValue {
        match lit {
            Literal::Numeric(n) => match n {
                NumericLiteral::Int(i) => RuntimeValue::Int(*i),
                NumericLiteral::Float(f) => RuntimeValue::Float(*f),
            },
            Literal::String(s) => RuntimeValue::String(s.clone()),
            Literal::Boolean(b) => RuntimeValue::Boolean(*b), // cloned
        }
    }
}

impl Operator {
    fn eval(&self, args: &[RuntimeValue]) -> RuntimeValue {
        args.iter()
            .cloned()
            .reduce(|acc, val| self.binary(acc, val)) // cannot return reference to temporary value returns a reference to data owned by the current functionrustcClick for full compiler diagnostic. temporary value created here
            .unwrap()
    }

    fn binary(&self, a: RuntimeValue, b: RuntimeValue) -> RuntimeValue {
        match self {
            Operator::Add => a + b,
            Operator::Div => a / b,
            Operator::Mul => a * b,
            Operator::Sub => a - b,
        }
    }
}

impl Node {
    fn eval(&self, scope: &Scope) -> RuntimeValue {
        match self {
            Node::List(list) => eval_list(list, scope),
            Node::Literal(lit) => RuntimeValue::from_literal(lit),
            Node::Op(op) => RuntimeValue::Op(*op),
            Node::Ident(ident) => scope
                .bindings
                .get(ident)
                .expect(format!("Identifier {ident} not found in scope").as_str())
                .clone(),
            Node::Fn => panic!("Node::Fn should not be evaluated"),
            Node::If => panic!("Node::If should not be evaluated"),
            Node::Let => panic!("Node::Let should not be evaluated"),
            Node::Quote => panic!("Node::Quote should not be evaluated"),
        }
    }
}

enum Form {
    Special(SpecialForm),
    Regular(Vec<Node>),
}

struct FnForm {
    args: Vec<String>,
    body: Node, // for example, we might want: (let (get-size (fn () 1)) (get-size))
}

struct IfForm {
    condition: Node,
    if_body: Node,
    else_body: Node,
}

struct LetForm {
    bindings: Vec<(String, Node)>,
    expr: Node,
}

struct QuoteForm {
    expr: Node,
}

enum SpecialForm {
    Fn(FnForm),
    If(IfForm),
    Let(LetForm),
    Quote(QuoteForm),
}

impl SpecialForm {
    fn eval(&self, scope: &Scope) -> RuntimeValue {
        match self {
            SpecialForm::Fn(form) => {
                // form.args
                // let args = parse_as_args(&form.args);
                // let fn_body = &form.body;

                // todo substitute scope into fn_body
                // let _ = scope;

                RuntimeValue::Function(Function {
                    params: form.args.clone(),
                    body: form.body.clone(),
                })
            }
            SpecialForm::If(form) => {
                if form.condition.eval(scope).bool() {
                    form.if_body.eval(scope)
                } else {
                    form.else_body.eval(scope)
                }
            }
            SpecialForm::Let(form) => {
                let bindings = form_let_bindings(&form.bindings, scope);
                form.expr.eval(&scope.with_bindings(bindings))
            }
            SpecialForm::Quote(form) => quote(&form.expr, scope),
        }
    }
}

/// structurally parses a list into a form so that structural checking can
/// be decoupled from evaluation
fn parse_form(list: &[Node]) -> Form {
    match list[0] {
        Node::Fn => {
            match &list[1..] {
                [Node::List(args), bodyexpr] => {
                    let args = args
                        .iter()
                        .map(|arg_node| match arg_node {
                            Node::Ident(ident) => ident.clone(),
                            _ => panic!("Function arguments must be identifiers"),
                        })
                        .collect();
                    Form::Special(SpecialForm::Fn(FnForm {
                        args,
                        body: bodyexpr.clone(),
                    }))
                }
                _ => panic!("Fn form must be called with a list of arguments and a body"),
            }
        }
        Node::If => {
            match &list[1..] {
                [condition, if_body, else_body] => {
                    Form::Special(SpecialForm::If(IfForm {
                        condition: condition.clone(),
                        if_body: if_body.clone(),
                        else_body: else_body.clone(),
                    }))
                }
                _ => panic!("If form must be called with a condition, if body, and else body"),
            }
        }
        Node::Let => {
            if list.len() < 3 {
                panic!("Let form must be called with a list of bindings and an expr");
            }
            let bindings = &list[1..list.len() - 1];
            let expr = list.last().unwrap();
            return Form::Special(SpecialForm::Let(LetForm {
                bindings: bindings
                    .iter()
                    .map(|node| {
                        match node {
                            Node::List(nodes) => {
                                if nodes.len() != 2 {
                                    panic!("let binding must be a list of two elements");
                                }
                                if let Node::Ident(ident) = &nodes[0] {
                                    (ident.clone(), nodes[1].clone())
                                } else {
                                    panic!("left side of let binding must be an identifier");
                                }
                            }
                            _ => panic!("All bindings must be lists"),
                        }
                    }).collect(),
                expr: expr.clone(),
            }));
        }
        Node::Quote => {
            match &list[1..] {
                [expr] => {
                    Form::Special(SpecialForm::Quote(QuoteForm {
                        expr: expr.clone(),
                    }))
                }
                _ => panic!("Quote form must be called with an expr"),
            }
        }
        _ => {
            Form::Regular(list.to_vec())
        }
    }
}

impl Form {
    fn eval(&self, scope: &Scope) -> RuntimeValue {
        match self {
            Form::Special(form) => form.eval(scope),
            Form::Regular(node) => eval_normal_form(node, scope)
        }
    }
}

fn eval_list(list: &Vec<Node>, scope: &Scope) -> RuntimeValue {
    parse_form(list).eval(scope)
}

fn eval_normal_form(list: &Vec<Node>, scope: &Scope) -> RuntimeValue {
    // let form = parse_form(list);

    let vals = list
        .iter()
        .map(|arg| arg.eval(scope))
        .collect::<Vec<RuntimeValue>>();

    let head_val = &vals[0];
    let args_vals = &vals[1..];

    match head_val {
        RuntimeValue::Op(op) => op.eval(args_vals),
        RuntimeValue::BuiltIn(builtin) => builtin.eval(args_vals),
        RuntimeValue::Function(func) => func.eval(args_vals, scope),
        RuntimeValue::Int(_) => panic!("Cannot call int value. list: {:?}", list),
        RuntimeValue::List(_) => panic!("Cannot call list value"),
        RuntimeValue::Float(_) => panic!("Cannot call float value"),
        RuntimeValue::String(_) => panic!("Cannot call string value"),
        RuntimeValue::Boolean(_) => panic!("Cannot call boolean value"),
        RuntimeValue::Symbol(_) => panic!("Cannot call symbol value"),
    }
}

fn eval_list_as_function_declaration(list: &Vec<Node>, scope: &Scope) -> RuntimeValue {
    let args = parse_as_args(&list[1]);
    let fn_body = &list[2];

    // todo substitute scope into fn_body
    let _ = scope;

    return RuntimeValue::Function(Function {
        params: args,
        body: fn_body.clone(),
    });
}

fn quote(node: &Node, scope: &Scope) -> RuntimeValue {
    match node {
        Node::List(list) => RuntimeValue::List(list.iter().map(|node| quote(node, scope)).collect()),
        Node::Ident(ident) => RuntimeValue::Symbol(ident.clone()),
        Node::Literal(_) => node.eval(scope),
        Node::Op(_) => todo!(),
        Node::Fn => todo!(),
        Node::If => todo!(),
        Node::Let => todo!(),
        Node::Quote => panic!("Cannot quote the quote special form"),
    }

}

fn form_let_bindings(list: &[(String, Node)], scope: &Scope) -> Vec<(String, RuntimeValue)> {
    list.iter()
        .cloned()
        .map(|(name, expr)| (name, expr.eval(scope)))
        .collect::<Vec<(String, RuntimeValue)>>()
}

/// for now, assume that the AST is a single SExpr
/// and just evaluate it.
/// Obvious next steps are to allow for multiple SExprs (lines)
/// and to manage a global scope being passed between them.
pub fn evaluate(ast: &AST) {
    println!("{:#?}", ast.root.eval(&Scope::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let ast = AST {
            root: Node::List(vec![
                Node::Op(Operator::Add),
                Node::Literal(Literal::Numeric(NumericLiteral::Int(1))),
                Node::Literal(Literal::Numeric(NumericLiteral::Int(2))),
            ]),
        };
        let output = ast.root.eval(&Scope::new());
        assert_eq!(output, RuntimeValue::Int(3));
    }

    #[test]
    fn test2() {
        let ast = AST {
            root: Node::List(vec![
                Node::Op(Operator::Add),
                Node::Literal(Literal::Numeric(NumericLiteral::Int(1))),
                Node::Literal(Literal::Numeric(NumericLiteral::Int(2))),
                Node::List(vec![
                    Node::Op(Operator::Sub),
                    Node::Literal(Literal::Numeric(NumericLiteral::Int(4))),
                    Node::Literal(Literal::Numeric(NumericLiteral::Int(3))),
                ]),
                Node::Literal(Literal::Numeric(NumericLiteral::Int(5))),
                Node::List(vec![
                    Node::Op(Operator::Mul),
                    Node::Literal(Literal::Numeric(NumericLiteral::Int(1))),
                    Node::Literal(Literal::Numeric(NumericLiteral::Float(2.3))),
                ]),
            ]),
        };
        let res = ast.root.eval(&Scope::new());
        assert_eq!(res, RuntimeValue::Float(11.3))
    }

    #[test]
    fn test3() {
        let ast = AST {
            root: Node::List(vec![
                Node::Let,
                Node::List(vec![
                    Node::Ident("x".to_owned()),
                    Node::Literal(Literal::Numeric(NumericLiteral::Int(2))),
                ]),
                Node::List(vec![
                    Node::Op(Operator::Mul),
                    Node::Ident("x".to_owned()),
                    Node::Literal(Literal::Numeric(NumericLiteral::Int(3))),
                ]),
            ]),
        };
        let res = ast.root.eval(&Scope::new());
        assert_eq!(res, RuntimeValue::Int(6))
    }
}