use nucleo_matcher::{
	Config, Matcher, Utf32Str,
	pattern::{CaseMatching, Normalization, Pattern},
};

pub struct Searcher {
	matcher: Matcher,
}

impl Default for Searcher {
	fn default() -> Self {
		return Self {
			matcher: Matcher::new(Config::DEFAULT),
		};
	}
}

impl Searcher {
	pub fn new() -> Self {
		return Self::default();
	}

	pub fn score(&mut self, query: impl AsRef<str>, haystack: impl AsRef<str>) -> Option<u32> {
		if query.as_ref().is_empty() {
			return Some(0);
		}

		let pattern = Pattern::parse(query.as_ref(), CaseMatching::Ignore, Normalization::Smart);
		let mut buf = Vec::new();
		return pattern.score(Utf32Str::new(haystack.as_ref(), &mut buf), &mut self.matcher);
	}
}
