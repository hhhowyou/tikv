// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! This crate provides a macro that can be used to append a match expression with multiple
//! arms, where token can be substituted for each arm.
//!
//! For example, the following code
//!
//! ```ignore
//! match_template! {
//!     T = [Int, Real, Double],
//!     match Foo {
//!         EvalType::T => { panic!("{}", EvalType::T); },
//!     }
//! }
//! ```
//!
//! generates
//!
//! ```ignore
//! match Foo {
//!     EvalType::Int => { panic!("{}", EvalType::Int); },
//!     EvalType::Real => { panic!("{}", EvalType::Real); },
//!     EvalType::Double => { panic!("{}", EvalType::Double); },
//! }
//! ```

#[macro_use]
extern crate quote;
extern crate proc_macro;

use proc_macro2::{TokenStream, TokenTree};
use syn::fold::Fold;
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::*;

struct MatchArmIdentFolder {
    from_ident: Ident,
    to_ident: Ident,
}

impl Fold for MatchArmIdentFolder {
    fn fold_macro(&mut self, i: Macro) -> Macro {
        let mut m = syn::fold::fold_macro(self, i);
        m.tts = m
            .tts
            .into_iter()
            .map(|tt| {
                if let TokenTree::Ident(i) = &tt {
                    if i == &self.from_ident {
                        return TokenTree::Ident(self.to_ident.clone());
                    }
                }
                tt
            })
            .collect();
        m
    }

    fn fold_ident(&mut self, i: Ident) -> Ident {
        if i == self.from_ident {
            self.to_ident.clone()
        } else {
            i
        }
    }
}

struct MatchTemplate {
    token_ident: Ident,
    substitutes_ident: Punctuated<Ident, Token![,]>,
    match_exp: Box<Expr>,
    match_arm: Arm,
}

impl Parse for MatchTemplate {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let token_ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let substitutes_content;
        bracketed!(substitutes_content in input);
        let substitutes_ident =
            Punctuated::<Ident, Token![,]>::parse_terminated(&substitutes_content)?;
        input.parse::<Token![,]>()?;
        let m: ExprMatch = input.parse()?;
        assert_eq!(m.arms.len(), 1, "Expect one match arm");
        let mut arm = m.arms.into_iter().next().unwrap();
        assert!(arm.guard.is_none(), "Expect no match arm guard");
        assert_eq!(arm.pats.len(), 1, "Expect one match arm pattern");
        arm.comma = None;
        Ok(Self {
            token_ident,
            substitutes_ident,
            match_exp: m.expr,
            match_arm: arm,
        })
    }
}

impl MatchTemplate {
    fn expand(self) -> TokenStream {
        let Self {
            token_ident,
            substitutes_ident,
            match_exp,
            match_arm,
        } = self;
        let match_arms = substitutes_ident.into_iter().map(|ident| {
            MatchArmIdentFolder {
                from_ident: token_ident.clone(),
                to_ident: ident,
            }
            .fold_arm(match_arm.clone())
        });
        quote! {
            match #match_exp {
                #(#match_arms,)*
            }
        }
    }
}

#[proc_macro]
pub fn match_template(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mt = parse_macro_input!(input as MatchTemplate);
    mt.expand().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let input = r#"
            T = [Int, Real, Double],
            match foo() {
                EvalType::T => { panic!("{}", EvalType::T); },
            }
        "#;

        let expect_output = r#"
            match foo() {
                EvalType::Int => { panic!("{}", EvalType::Int); },
                EvalType::Real => { panic!("{}", EvalType::Real); },
                EvalType::Double => { panic!("{}", EvalType::Double); },
            }
        "#;
        let expect_output_stream: TokenStream = expect_output.parse().unwrap();

        let mt: MatchTemplate = syn::parse_str(input).unwrap();
        let output = mt.expand();
        assert_eq!(output.to_string(), expect_output_stream.to_string());
    }
}
