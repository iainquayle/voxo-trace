use std::collections::HashMap;

macro_rules! unique_index {() => {line!()};}
pub(crate) use unique_index;

macro_rules! map_constants { ($($constant:ident),* $(,[ $format_string:literal; $($format_constant:ident),*]),*) => {
		{
			let mut replacements: Vec<(String, Option<String>)> = Vec::new();
			$(
				replacements.push((stringify!($constant).to_string(), Some($constant.to_string())));
			)*
			$($(
				replacements.push((stringify!($format_constant).to_string(), Some(format!($format_string, $format_constant).to_string())));
			)+)+
			replacements
		} 
	};
}
pub(crate) use map_constants;

pub fn shader_preprocessor(source: String, definitions: Vec<(String, Option<String>)>) -> String {
	//processor will replace anything where: /*MATCHES_DEF_NAME*/to be replaces if name exists/**/
	//can use indexes so long as all have been replaced before the next one is replaced
	let definitions_map: HashMap<_, _> = HashMap::from_iter(definitions);
	println!("{:?}", definitions_map);

	let mut new_source = String::new();
	let mut is_opened = false;

	for section in source.split("/*") {
		//println!("{}", section);
		let (id, back)	= section.split_once("*/").unwrap_or((section, ""));
		match definitions_map.get(id) {
			Some(definition) => {
				if !is_opened {
					new_source.push_str(definition.clone().unwrap_or_default().as_str());
					is_opened = true;
				} else {
					new_source.push_str(section);
				}
			},
			None => {
				if is_opened {
					new_source.push_str(back);
					is_opened = false;
				} else {
					new_source.push_str(section);	
				}
			},
		}
	}
	new_source
}