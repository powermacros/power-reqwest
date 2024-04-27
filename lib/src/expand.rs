use quote::ToTokens;

use crate::model::Client;

impl ToTokens for Client {
    fn to_tokens(&self, _tokens: &mut proc_macro2::TokenStream) {}
}
