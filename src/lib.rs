use power_reqwest_lib::Client;
use quote::ToTokens;

#[proc_macro]
pub fn reqwest(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match syn::parse::<Client>(input) {
        Ok(client) => client.to_token_stream().into(),
        Err(err) => err.to_compile_error().into(),
    }
}
