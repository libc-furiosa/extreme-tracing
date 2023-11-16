use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::ToTokens;
use syn::ext::IdentExt;
use syn::{
    parse::{Parse, ParseStream, Parser},
    parse_quote,
    punctuated::Punctuated,
    Expr, Ident, LitInt, LitStr, Path, Token,
};

#[derive(Default)]
struct ChromeEventArgs {
    level: Option<Level>,
    target: Option<LitStr>,
    event: Option<Event>,
    fields: Fields,
    skips: HashSet<Ident>,
}

#[derive(Default)]
struct Fields(Punctuated<Field, Token![,]>);

impl ToTokens for Fields {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}

impl Parse for Fields {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let _ = input.parse::<kw::fields>();
        let content;
        let _ = syn::parenthesized!(content in input);
        let fields: Punctuated<_, Token![,]> = content.parse_terminated(Field::parse)?;
        Ok(Self(fields))
    }
}

#[derive(Clone)]
enum Level {
    Str(LitStr),
    Int(LitInt),
    Path(Path),
}

impl Parse for Level {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let _ = input.parse::<kw::level>()?;
        let _ = input.parse::<Token![=]>()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(LitStr) {
            Ok(Self::Str(input.parse()?))
        } else if lookahead.peek(LitInt) {
            Ok(Self::Int(input.parse()?))
        } else if lookahead.peek(Ident) {
            Ok(Self::Path(input.parse()?))
        } else {
            Err(lookahead.error())
        }
    }
}

struct Skips(HashSet<Ident>);

impl Parse for Skips {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let _ = input.parse::<kw::skip>();
        let content;
        let _ = syn::parenthesized!(content in input);
        let names: Punctuated<Ident, Token![,]> = content.parse_terminated(Ident::parse_any)?;
        let mut skips = HashSet::new();
        for name in names {
            if skips.contains(&name) {
                return Err(syn::Error::new(
                    name.span(),
                    "tried to skip the same field twice",
                ));
            } else {
                skips.insert(name);
            }
        }
        Ok(Self(skips))
    }
}

impl ToTokens for ChromeEventArgs {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.fields.to_tokens(tokens);
    }
}

impl Parse for ChromeEventArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut args = Self::default();

        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::event) {
                args.event = Some(Event::parse(input)?);
            } else if lookahead.peek(kw::level) {
                args.level = Some(Level::parse(input)?)
            } else if lookahead.peek(kw::fields) {
                args.fields = Fields::parse(input)?;
            } else if lookahead.peek(kw::skip) {
                let Skips(skips) = input.parse()?;
                args.skips = skips;
            } else if lookahead.peek(kw::target) {
                let target = input.parse::<StrArg<kw::target>>()?.value;
                args.target = Some(target);
            } else if lookahead.peek(Token![,]) {
                let _ = input.parse::<Token![,]>()?;
            } else {
                panic!(
                    "Unknown fields, expected one of \"level\", \"fields\", \"skip\", \"target\"",
                )
            }
        }
        Ok(args)
    }
}

struct StrArg<T> {
    value: LitStr,
    _p: std::marker::PhantomData<T>,
}

impl<T: Parse> Parse for StrArg<T> {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let _ = input.parse::<T>()?;
        let _ = input.parse::<Token![=]>()?;
        let value = input.parse()?;
        Ok(Self {
            value,
            _p: std::marker::PhantomData,
        })
    }
}

struct Event {
    keyword: kw::event,
    colon_token: Token![:],
    event: LitStr,
    comma_token: Token![,],
}

impl Parse for Event {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Event {
            keyword: input.parse()?,
            colon_token: input.parse()?,
            event: input.parse()?,
            comma_token: input.parse()?,
        })
    }
}

impl ToTokens for Event {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.keyword.to_tokens(tokens);
        self.colon_token.to_tokens(tokens);
        self.event.to_tokens(tokens);
        self.comma_token.to_tokens(tokens);
    }
}

struct Field {
    path: Path,
    eq_token: Token![=],
    value: Expr,
}

impl Parse for Field {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Field {
            path: input.parse()?,
            eq_token: input.parse()?,
            value: input.parse()?,
        })
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.path.to_tokens(tokens);
        self.eq_token.to_tokens(tokens);
        self.value.to_tokens(tokens);
    }
}

#[proc_macro_attribute]
pub fn instrument(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as ChromeEventArgs);

    let event = args
        .event
        .map(|e| e.event)
        .unwrap_or_else(|| LitStr::new("", Span::call_site()));

    let fields = args.fields.0.iter();
    let fields2 = args.fields.0.iter();
    let fields3 = args.fields.0.iter();

    let mut input = syn::Item::parse.parse(item).unwrap();

    if let syn::Item::Fn(ref mut item) = input {
        let original = &item.block;
        item.block = Box::new(parse_quote! {{
            let start = chrometracer::current(|tracer| tracer.map(|t| t.start));

            if let Some(start) = start {
                let event = match #event {
                    "async" => Some((chrometracer::EventType::AsyncStart, chrometracer::EventType::AsyncEnd)),
                    "" => None,
                    _ => panic!("Unknown event, expected one of \"async\"")
                };

                let ret = if let Some(event) = event {
                    chrometracer::event!(#(#fields,)* ph = event.0,
                        ts = ::std::time::SystemTime::now().duration_since(start).unwrap().as_nanos() as f64 / 1000.0);
                    let ret = #original;
                    chrometracer::event!(#(#fields2,)* ph = event.1,
                        ts = ::std::time::SystemTime::now().duration_since(start).unwrap().as_nanos() as f64 / 1000.0);
                    ret
                } else {
                    let now = ::std::time::SystemTime::now();
                    let ts = now.duration_since(start).unwrap().as_nanos() as f64 / 1000.0;
                    let ret = #original;
                    let dur = ::std::time::SystemTime::now().duration_since(now).unwrap().as_nanos() as f64 / 1000.0;

                    chrometracer::event!(#(#fields3,)* ph = chrometracer::EventType::Complete, dur = dur, ts = ts);
                    ret
                };

                ret
            } else {
                #original
            }
        }});
    } else {
        unreachable!()
    }

    input.into_token_stream().into()
}

mod kw {
    syn::custom_keyword!(event);
    syn::custom_keyword!(skip);
    syn::custom_keyword!(fields);
    syn::custom_keyword!(level);
    syn::custom_keyword!(target);
}