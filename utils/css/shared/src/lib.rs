mod parse;

pub mod config;
pub mod hasher;

use std::borrow::Cow;
use std::collections::HashSet;

use nameth::NamedEnumValues as _;
use nameth::nameth;
pub use parse::parse_scss;

use crate::parse::Global;
use crate::parse::ScssFragment;

pub fn rewrite_classes(
    scss_file: &str,
    rename: impl Fn(&str) -> String,
) -> Result<String, ScssError> {
    let fragments =
        parse::parse_scss(scss_file).map_err(|error| ScssError::ParseError(error.to_string()))?;
    let mut new_file = String::with_capacity(scss_file.len() * 2);
    let mut cursor = scss_file;

    for fragment in fragments {
        let (span, replace) = match fragment {
            ScssFragment::Class(class) => (class, Cow::Owned(rename(class))),
            ScssFragment::Global(Global { inner, outer }) => (outer, Cow::Borrowed(inner)),
        };

        let (before, after) = cursor.split_at(span.as_ptr() as usize - cursor.as_ptr() as usize);
        cursor = &after[span.len()..];
        new_file.push_str(before);
        new_file.push_str(&replace);
    }
    new_file.push_str(cursor);

    Ok(new_file)
}

pub fn list_classes(scss_file: &str) -> Result<impl Iterator<Item = &str>, ScssError> {
    let fragments =
        parse::parse_scss(scss_file).map_err(|error| ScssError::ParseError(error.to_string()))?;
    let mut seen = HashSet::with_capacity(fragments.len());
    Ok(fragments.into_iter().filter_map(move |fragment| {
        if let ScssFragment::Class(class) = fragment
            && seen.insert(class)
        {
            Some(class)
        } else {
            None
        }
    }))
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ScssError {
    #[error("[{n}] Failed to parse SCSS: {0}", n = self.name())]
    ParseError(String),
}

#[cfg(test)]
mod tests {

    const TEST_CASE: &str = r#"
@charset "utf-8";

// Line comment

/* block comment 
.class {
	{ // Invalid syntax inside the comment to ensure this isn't parsed.
}
*/

div.style1.style2[value="thing"] {
	color: red; // line comment after declaration
	background-color: black;
}

.style-with-dashes {
	color: red;
}

@media (max-width: 600px) {
	.style3 {
		background-color: #87ceeb;
	}
}

@font-face {
	font-family: "Trickster";
	src:
		local("Trickster"),
		url("trickster-COLRv1.otf") format("opentype") tech(color-COLRv1),
		url("trickster-outline.otf") format("opentype"),
		url("trickster-outline.woff") format("woff");
}

div > .style4 {
	.nested-style {
		color: red;
	}

	@media (max-width: 600px) {
		// scss style nested media query with declarations inside
		color: blue;
	}
}

:global(.global-class) {
	color: red;
}

$some-scss-variable: 10px; // Scss variable declarations in top scope.

.style5 // comment in between selector

.style6

/* comment in between selector */
.style7 {
	$some-scss-variable: 10px; // Scss variable declarations inside style rules block.
	color: red;
}

.style1 {
	// Repeated style
	color: blue;
}

// scss placeholder class should parse
%scss-class {
	color: red;
}

@layer test-layer {
	.style8 {
		color: red;
	}
}

@layer test-layer2;

@layer;

@container (min-width: #{$screen-md}) {
	h2 {
		font-size: 1.5em;
	}

	.style9 {
		font-size: 1.5em;
	}
}

//eof
"#;

    #[test]
    fn rewrite_classes() {
        let actual = super::rewrite_classes(TEST_CASE, |c| format!("{c}-REWRITE")).unwrap();
        let expected = r#"
@charset "utf-8";

// Line comment

/* block comment 
.class {
	{ // Invalid syntax inside the comment to ensure this isn't parsed.
}
*/

div.style1-REWRITE.style2-REWRITE[value="thing"] {
	color: red; // line comment after declaration
	background-color: black;
}

.style-with-dashes-REWRITE {
	color: red;
}

@media (max-width: 600px) {
	.style3-REWRITE {
		background-color: #87ceeb;
	}
}

@font-face {
	font-family: "Trickster";
	src:
		local("Trickster"),
		url("trickster-COLRv1.otf") format("opentype") tech(color-COLRv1),
		url("trickster-outline.otf") format("opentype"),
		url("trickster-outline.woff") format("woff");
}

div > .style4-REWRITE {
	.nested-style-REWRITE {
		color: red;
	}

	@media (max-width: 600px) {
		// scss style nested media query with declarations inside
		color: blue;
	}
}

.global-class {
	color: red;
}

$some-scss-variable: 10px; // Scss variable declarations in top scope.

.style5-REWRITE // comment in between selector

.style6-REWRITE

/* comment in between selector */
.style7-REWRITE {
	$some-scss-variable: 10px; // Scss variable declarations inside style rules block.
	color: red;
}

.style1-REWRITE {
	// Repeated style
	color: blue;
}

// scss placeholder class should parse
%scss-class {
	color: red;
}

@layer test-layer {
	.style8-REWRITE {
		color: red;
	}
}

@layer test-layer2;

@layer;

@container (min-width: #{$screen-md}) {
	h2 {
		font-size: 1.5em;
	}

	.style9-REWRITE {
		font-size: 1.5em;
	}
}

//eof
"#;
        assert_eq!(expected, actual);
    }

    #[test]
    fn list_classes() {
        assert_eq!(
            vec![
                "style1",
                "style2",
                "style-with-dashes",
                "style3",
                "style4",
                "nested-style",
                "style5",
                "style6",
                "style7",
                "style8",
                "style9"
            ],
            super::list_classes(TEST_CASE).unwrap().collect::<Vec<_>>()
        )
    }
}
