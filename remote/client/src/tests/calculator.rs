use tonic::Status;
use tonic::async_trait;
use tracing::info;
use trz_gateway_common::protos::terrazzo::remote::tests::Expression;
use trz_gateway_common::protos::terrazzo::remote::tests::Operator;
use trz_gateway_common::protos::terrazzo::remote::tests::Value;
use trz_gateway_common::protos::terrazzo::remote::tests::expression;
use trz_gateway_common::protos::terrazzo::remote::tests::test_tunnel_service_server::TestTunnelService;
use trz_gateway_common::protos::terrazzo::remote::tests::value;

pub struct Calculator;

#[async_trait]
impl TestTunnelService for Calculator {
    async fn calculate(
        &self,
        request: tonic::Request<Expression>,
    ) -> Result<tonic::Response<Value>, Status> {
        let expression = request.get_ref();
        let result = calculate_impl(expression);
        let result_str = match &result {
            Ok(result) => result.to_string(),
            Err(error) => error.to_string(),
        };
        info!("Calculate {expression} = {result_str}");
        result.map(tonic::Response::new)
    }
}

fn calculate_impl(request: &Expression) -> Result<Value, Status> {
    let result = match request
        .kind
        .as_ref()
        .ok_or_else(|| Status::invalid_argument("null"))?
    {
        expression::Kind::Operation(operation) => {
            let operands = [&operation.left, &operation.right]
                .map(Option::as_ref)
                .map(|e| e.ok_or_else(|| Status::invalid_argument("null operand")))
                .map(|e| e.map(|e| calculate_impl(&*e)));
            let [a, b] = operands;
            let operands = [a??, b??].map(|e| {
                e.kind
                    .ok_or_else(|| Status::invalid_argument("null result"))
            });
            let [a, b] = operands;
            let operands = (a?, b?);
            match operation.operator() {
                Operator::UndefinedOperand => {
                    return Err(Status::invalid_argument("null operator"));
                }
                Operator::Plus => match operands {
                    (value::Kind::I(a), value::Kind::I(b)) => (a + b).into(),
                    (value::Kind::I(a), value::Kind::F(b)) => (a as f64 + b).into(),
                    (value::Kind::F(a), value::Kind::I(b)) => (a + b as f64).into(),
                    (value::Kind::F(a), value::Kind::F(b)) => (a + b).into(),
                },
                Operator::Minus => match operands {
                    (value::Kind::I(a), value::Kind::I(b)) => (a - b).into(),
                    (value::Kind::I(a), value::Kind::F(b)) => (a as f64 - b).into(),
                    (value::Kind::F(a), value::Kind::I(b)) => (a - b as f64).into(),
                    (value::Kind::F(a), value::Kind::F(b)) => (a - b).into(),
                },
                Operator::Multiply => match operands {
                    (value::Kind::I(a), value::Kind::I(b)) => (a * b).into(),
                    (value::Kind::I(a), value::Kind::F(b)) => (a as f64 * b).into(),
                    (value::Kind::F(a), value::Kind::I(b)) => (a * b as f64).into(),
                    (value::Kind::F(a), value::Kind::F(b)) => (a * b).into(),
                },
                Operator::Divide => match operands {
                    (value::Kind::I(a), value::Kind::I(b)) => (a / b).into(),
                    (value::Kind::I(a), value::Kind::F(b)) => (a as f64 / b).into(),
                    (value::Kind::F(a), value::Kind::I(b)) => (a / b as f64).into(),
                    (value::Kind::F(a), value::Kind::F(b)) => (a / b).into(),
                },
            }
        }
        expression::Kind::Value(value) => value.clone(),
    };
    Ok(result)
}
