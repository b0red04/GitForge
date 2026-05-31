use fluent_bundle::{FluentBundle, FluentResource};
use std::sync::Arc;

fn en_us_ftl() -> &'static str {
    include_str!("../../../../assets/lang/en-US.ftl")
}

pub struct Localization {
    bundle: Arc<FluentBundle<FluentResource>>,
}

impl Localization {
    pub fn new() -> Self {
        let locale = sys_locale::get_locale()
            .unwrap_or_else(|| "en-US".into());

        let lang_id = locale.parse().unwrap_or_else(|_| {
            "en-US".parse().unwrap()
        });

        let ftl_string = en_us_ftl();
        let resource = FluentResource::try_new(ftl_string.to_string())
            .expect("Failed to parse en-US.ftl");

        let mut bundle = FluentBundle::new(vec![lang_id]);
        bundle.add_resource(resource)
            .expect("Failed to add FTL resource");

        Self {
            bundle: Arc::new(bundle),
        }
    }

    pub fn t(&self, key: &str) -> String {
        let msg = self.bundle.get_message(key);
        let Some(msg) = msg else {
            return key.to_string();
        };

        let value = msg.value();
        let Some(pattern) = value else {
            return key.to_string();
        };

        let mut errors = vec![];
        self.bundle.format_pattern(pattern, None, &mut errors)
            .to_string()
    }

    pub fn t_with_args(&self, key: &str, args: &[(impl AsRef<str>, impl AsRef<str>)]) -> String {
        use fluent_bundle::FluentArgs;
        let msg = self.bundle.get_message(key);
        let Some(msg) = msg else {
            return key.to_string();
        };

        let value = msg.value();
        let Some(pattern) = value else {
            return key.to_string();
        };

        let mut fluent_args = FluentArgs::new();
        for (k, v) in args {
            fluent_args.set(k.as_ref(), v.as_ref());
        }

        let mut errors = vec![];
        self.bundle.format_pattern(pattern, Some(&fluent_args), &mut errors)
            .to_string()
    }
}

impl Default for Localization {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Localization {
    fn clone(&self) -> Self {
        Self {
            bundle: Arc::clone(&self.bundle),
        }
    }
}
