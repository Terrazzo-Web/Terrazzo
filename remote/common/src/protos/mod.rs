pub mod terrazzo {
    pub mod remote {
        pub mod health {
            include!(concat!(env!("OUT_DIR"), "/terrazzo.remote.health.rs"));
        }

        #[cfg(debug_assertions)]
        pub mod tests {
            use std::fmt::Debug;
            use std::fmt::Display;

            include!(concat!(env!("OUT_DIR"), "/terrazzo.remote.tests.rs"));

            impl From<i64> for Expression {
                fn from(value: i64) -> Self {
                    Self {
                        kind: Some(expression::Kind::Value(value.into())),
                    }
                }
            }

            impl From<f64> for Expression {
                fn from(value: f64) -> Self {
                    Self {
                        kind: Some(expression::Kind::Value(value.into())),
                    }
                }
            }

            impl From<i64> for Value {
                fn from(value: i64) -> Self {
                    Self {
                        kind: Some(value::Kind::I(value)),
                    }
                }
            }

            impl From<f64> for Value {
                fn from(value: f64) -> Self {
                    Self {
                        kind: Some(value::Kind::F(value)),
                    }
                }
            }

            impl std::ops::Add for Expression {
                type Output = Expression;

                fn add(self, rhs: Self) -> Self::Output {
                    Self::new(self, Operator::Plus, rhs)
                }
            }

            impl std::ops::Sub for Expression {
                type Output = Expression;

                fn sub(self, rhs: Self) -> Self::Output {
                    Self::new(self, Operator::Minus, rhs)
                }
            }

            impl std::ops::Mul for Expression {
                type Output = Expression;

                fn mul(self, rhs: Self) -> Self::Output {
                    Self::new(self, Operator::Multiply, rhs)
                }
            }

            impl std::ops::Div for Expression {
                type Output = Expression;

                fn div(self, rhs: Self) -> Self::Output {
                    Self::new(self, Operator::Divide, rhs)
                }
            }

            impl Expression {
                pub fn new(left: Expression, operator: Operator, right: Expression) -> Self {
                    Self {
                        kind: Some(expression::Kind::Operation(Box::new(Operation {
                            left: Some(Box::new(left)),
                            operator: operator as i32,
                            right: Some(Box::new(right)),
                        }))),
                    }
                }
            }

            impl Display for Expression {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let Expression { kind } = self;
                    let Some(kind) = kind else {
                        return Display::fmt("NULL", f);
                    };
                    match kind {
                        expression::Kind::Operation(operation) => Display::fmt(operation, f),
                        expression::Kind::Value(value) => Display::fmt(value, f),
                    }
                }
            }

            impl Display for Value {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let Value { kind } = self;
                    let Some(kind) = kind else {
                        return Display::fmt("NULL", f);
                    };
                    match kind {
                        value::Kind::I(v) => Display::fmt(v, f),
                        value::Kind::F(v) => Display::fmt(v, f),
                    }
                }
            }

            impl Display for Operation {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match &self.left {
                        Some(left) => Display::fmt(left, f)?,
                        None => Display::fmt("NULL", f)?,
                    }
                    Display::fmt(&self.operator(), f)?;
                    match &self.right {
                        Some(right) => Display::fmt(right, f)?,
                        None => Display::fmt("NULL", f)?,
                    }
                    Ok(())
                }
            }

            impl Display for Operator {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let str = match self {
                        Operator::UndefinedOperand => " UNDEFINED ",
                        Operator::Plus => " + ",
                        Operator::Minus => " - ",
                        Operator::Multiply => " * ",
                        Operator::Divide => " / ",
                    };
                    Display::fmt(str, f)
                }
            }
        }
    }
}
