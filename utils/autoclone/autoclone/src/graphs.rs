mod function;
mod graph;
mod named_inputs;
mod return_type;
mod tests;

pub fn graph2(
    _attr: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    let mut graph = graph::Graph::new(item)?;
    graph.record_functions()?;
    graph.process_functions();
    graph.into_token_stream()
}
