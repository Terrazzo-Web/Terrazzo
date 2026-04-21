mod parse;

// pub fn load_and_modify_css(
//     css_file: &str,
//     rename: impl Fn(&str) -> String,
// ) -> anyhow::Result<ModifyCssResult> {
//     let mut new_file = String::with_capacity(css_file.len() * 2);
//     let mut css_file_contents = css_file;

//     for fragment in fragments {
//         let (span, replace) = match fragment {
//             CssFragment::Class(class) => (
//                 class,
//                 Cow::Owned(config.class_name_pattern.apply(class, &hash_str)),
//             ),
//             CssFragment::Global(Global { inner, outer }) => (outer, Cow::Borrowed(inner)),
//         };

//         let (before, after) = cursor.split_at(span.as_ptr() as usize - cursor.as_ptr() as usize);
//         cursor = &after[span.len()..];
//         new_file.push_str(before);
//         new_file.push_str(&replace);
//     }

//     new_file.push_str(cursor);

//     Ok(ModifyCssResult {
//         path: css_file,
//         relative_path,
//         hash: hash_str,
//         contents: new_file,
//     })
// }
