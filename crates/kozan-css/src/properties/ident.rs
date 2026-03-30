//! CSS `<custom-ident>` and `url()` parsers.

use cssparser::Parser;
use kozan_style::{Atom, Ident, Url};
use crate::Error;

impl crate::Parse for Atom {
    /// Parses any CSS `<custom-ident>`.
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let ident = input.expect_ident()?;
        Ok(Atom::new(&*ident))
    }
}

impl crate::Parse for Ident {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let ident = input.expect_ident()?;
        Ok(Ident(Atom::new(&*ident)))
    }
}

impl crate::Parse for Url {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let url = input.expect_url()?;
        Ok(Url(Atom::new(&*url)))
    }
}
