use proc_macro2::TokenTree;
use quote::ToTokens;
use syn::{
    meta::ParseNestedMeta, parenthesized, parse::Parse, parse_quote,
    punctuated::Punctuated, token, AttrStyle, DeriveInput, Error, Ident,
    LitStr, MacroDelimiter, Meta, MetaList, Path, Token, WherePredicate,
};

fn try_set_attribute<T: ToTokens>(
    attribute: &mut Option<T>,
    value: T,
    name: &'static str,
) -> Result<(), Error> {
    if attribute.is_none() {
        *attribute = Some(value);
        Ok(())
    } else {
        Err(Error::new_spanned(
            value,
            format!("{} already specified", name),
        ))
    }
}

#[derive(Default)]
pub struct Attributes {
    pub archive_as: Option<LitStr>,
    pub archived: Option<Ident>,
    pub resolver: Option<Ident>,
    pub attrs: Vec<Meta>,
    pub compares: Option<Punctuated<Path, Token![,]>>,
    pub archive_bounds: Option<Punctuated<WherePredicate, Token![,]>>,
    pub serialize_bounds: Option<Punctuated<WherePredicate, Token![,]>>,
    pub deserialize_bounds: Option<Punctuated<WherePredicate, Token![,]>>,
    pub check_bytes: Option<Meta>,
    pub crate_path: Option<Path>,
}

impl Attributes {
    fn parse_meta(&mut self, meta: ParseNestedMeta<'_>) -> Result<(), Error> {
        if meta.path.is_ident("check_bytes") {
            let meta = if meta.input.peek(token::Paren) {
                let (delimiter, tokens) = meta.input.step(|cursor| {
                    if let Some((TokenTree::Group(g), rest)) =
                        cursor.token_tree()
                    {
                        Ok((
                            (
                                MacroDelimiter::Paren(token::Paren(
                                    g.delim_span(),
                                )),
                                g.stream(),
                            ),
                            rest,
                        ))
                    } else {
                        Err(cursor.error("expected delimiter"))
                    }
                })?;
                Meta::List(MetaList {
                    path: meta.path,
                    delimiter,
                    tokens,
                })
            } else {
                Meta::Path(meta.path)
            };

            try_set_attribute(&mut self.check_bytes, meta, "check_bytes")
        } else if meta.path.is_ident("compare") {
            let traits;
            parenthesized!(traits in meta.input);
            let traits = traits.parse_terminated(Path::parse, Token![,])?;
            try_set_attribute(&mut self.compares, traits, "compare")
        } else if meta.path.is_ident("archive_bounds") {
            let bounds;
            parenthesized!(bounds in meta.input);
            let clauses =
                bounds.parse_terminated(WherePredicate::parse, Token![,])?;
            try_set_attribute(
                &mut self.archive_bounds,
                clauses,
                "archive_bounds",
            )
        } else if meta.path.is_ident("serialize_bounds") {
            let bounds;
            parenthesized!(bounds in meta.input);
            let clauses =
                bounds.parse_terminated(WherePredicate::parse, Token![,])?;
            try_set_attribute(
                &mut self.serialize_bounds,
                clauses,
                "serialize_bounds",
            )
        } else if meta.path.is_ident("deserialize_bounds") {
            let bounds;
            parenthesized!(bounds in meta.input);
            let clauses =
                bounds.parse_terminated(WherePredicate::parse, Token![,])?;
            try_set_attribute(
                &mut self.deserialize_bounds,
                clauses,
                "deserialize_bounds",
            )
        } else if meta.path.is_ident("archived") {
            try_set_attribute(
                &mut self.archived,
                meta.value()?.parse()?,
                "archived",
            )
        } else if meta.path.is_ident("resolver") {
            try_set_attribute(
                &mut self.resolver,
                meta.value()?.parse()?,
                "resolver",
            )
        } else if meta.path.is_ident("as") {
            try_set_attribute(
                &mut self.archive_as,
                meta.value()?.parse()?,
                "as",
            )
        } else if meta.path.is_ident("crate") {
            if meta.input.parse::<Token![=]>().is_ok() {
                let path = meta.input.parse::<Path>()?;
                try_set_attribute(&mut self.crate_path, path, "crate")
            } else if meta.input.is_empty() || meta.input.peek(Token![,]) {
                try_set_attribute(
                    &mut self.crate_path,
                    parse_quote! { crate },
                    "crate",
                )
            } else {
                Err(meta.error("expected `crate` or `crate = ...`"))
            }
        } else if meta.path.is_ident("derive") {
            let metas;
            parenthesized!(metas in meta.input);
            self.attrs.extend(
                metas
                    .parse_terminated(Meta::parse, Token![,])?
                    .into_iter()
                    .map(|meta| parse_quote! { derive(#meta) }),
            );
            Ok(())
        } else if meta.path.is_ident("attr") {
            let metas;
            parenthesized!(metas in meta.input);
            self.attrs
                .extend(metas.parse_terminated(Meta::parse, Token![,])?);
            Ok(())
        } else {
            Err(meta.error("unrecognized archive argument"))
        }
    }

    pub fn parse(input: &DeriveInput) -> Result<Attributes, Error> {
        let mut result = Attributes::default();
        for attr in input.attrs.iter() {
            if !matches!(attr.style, AttrStyle::Outer) {
                continue;
            }

            if attr.path().is_ident("archive") || attr.path().is_ident("rkyv") {
                attr.parse_nested_meta(|meta| result.parse_meta(meta))?;
            } else if attr.path().is_ident("archive_attr")
                || attr.path().is_ident("rkyv_attr")
            {
                result.attrs.extend(
                    attr.parse_args_with(
                        Punctuated::<Meta, Token![,]>::parse_terminated,
                    )?
                    .into_iter(),
                );
            } else if attr.path().is_ident("rkyv_derive") {
                result.attrs.extend(
                    attr.parse_args_with(
                        Punctuated::<Meta, Token![,]>::parse_terminated,
                    )?
                    .into_iter()
                    .map(|meta| parse_quote! { derive(#meta) }),
                );
            }
        }

        if result.archive_as.is_some() && result.check_bytes.is_some() {
            Err(Error::new_spanned(
                result.check_bytes.unwrap(),
                "cannot generate a `CheckBytes` impl because `as = \"..\"` \
                 does not generate an archived type",
            ))
        } else {
            Ok(result)
        }
    }

    pub fn crate_path(&self) -> Path {
        self.crate_path
            .clone()
            .unwrap_or_else(|| parse_quote! { ::rkyv })
    }
}
