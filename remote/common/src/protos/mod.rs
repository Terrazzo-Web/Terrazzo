pub mod terrazzo {
    pub mod remote {
        pub mod health {
            include!(concat!(env!("OUT_DIR"), "/terrazzo.remote.health.rs"));
        }

        #[cfg(debug_assertions)]
        pub mod tests {

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
        }
    }
}
