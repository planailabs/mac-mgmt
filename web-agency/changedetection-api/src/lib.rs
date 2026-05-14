#[allow(unused_imports)]
use progenitor_client::{encode_path, ClientHooks, OperationInfo, RequestBuilderExt};
#[allow(unused_imports)]
pub use progenitor_client::{ByteStream, ClientInfo, Error, ResponseValue};
/// Types used as operation parameters and responses.
#[allow(clippy::all)]
pub mod types {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

        impl ::std::fmt::Debug for ConversionError {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }

        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }

        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }

    ///`CreateTag`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/Tag"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "title"
    ///      ]
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateTag {
        ///HTTP request body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub body: ::std::option::Option<CreateTagBody>,
        ///Browser automation steps. Maximum 100 steps allowed.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub browser_steps: ::std::vec::Vec<CreateTagBrowserStepsItem>,
        ///Compare against all history for unique lines
        #[serde(default)]
        pub check_unique_lines: bool,
        ///Array of condition rules for change detection logic (empty array
        /// when not set)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub conditions: ::std::vec::Vec<CreateTagConditionsItem>,
        ///Logic operator - ALL (match all conditions) or ANY (match any
        /// condition)
        #[serde(default = "defaults::create_tag_conditions_match_logic")]
        pub conditions_match_logic: CreateTagConditionsMatchLogic,
        ///Unix timestamp of creation
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub date_created: ::std::option::Option<i64>,
        ///Keep only lines containing these substrings (plain text,
        /// case-insensitive) — simpler alternative to regex
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_lines_containing: ::std::vec::Vec<CreateTagExtractLinesContainingItem>,
        ///Regex patterns to extract specific text after filtering
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_text: ::std::vec::Vec<CreateTagExtractTextItem>,
        ///Backend to use for fetching content. Common values:
        /// - `system` (default) - Use the system-wide default fetcher
        /// - `html_requests` - Fast requests-based fetcher
        /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
        /// - `extra_browser_*` - Custom browser configurations (if configured)
        /// - Plugin-provided fetchers (if installed)
        #[serde(default = "defaults::create_tag_fetch_backend")]
        pub fetch_backend: CreateTagFetchBackend,
        ///Send notification when filters fail to match content
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_failure_notification_send: bool,
        ///Include added text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_added: bool,
        ///Include removed text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_removed: bool,
        ///Include replaced text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_replaced: bool,
        ///Monitor and track price changes (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub follow_price_changes: bool,
        ///Whether page has LD-JSON price data (auto-detected)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub has_ldjson_price_data: ::std::option::Option<bool>,
        ///HTTP headers to include in requests
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub headers: ::std::collections::HashMap<::std::string::String, ::std::string::String>,
        ///Maximum number of history snapshots to keep (null = use system
        /// default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub history_snapshot_max_length: ::std::option::Option<::std::num::NonZeroU64>,
        ///Ignore HTTP status code errors (boolean or null)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ignore_status_codes: ::std::option::Option<bool>,
        ///Text patterns to ignore in change detection
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub ignore_text: ::std::vec::Vec<CreateTagIgnoreTextItem>,
        ///Only trigger on in-stock transitions (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub in_stock_only: bool,
        ///CSS/XPath selectors to extract specific content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub include_filters: ::std::vec::Vec<CreateTagIncludeFiltersItem>,
        ///Internal cache of AI evaluation results keyed by (intent, diff) hash
        /// (auto-managed).
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub llm_evaluation_cache: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        ///Number of tokens consumed by the AI on the most recent check.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_last_tokens_used: ::std::option::Option<i64>,
        ///CSS selector derived by the AI to narrow content scope before
        /// evaluation (auto-managed, do not set manually).
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_prefilter: ::std::option::Option<::std::string::String>,
        ///Total tokens consumed by the AI across all checks for this watch.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_tokens_used_cumulative: ::std::option::Option<i64>,
        ///HTTP method to use
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub method: ::std::option::Option<CreateTagMethod>,
        ///Custom notification body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_body: ::std::option::Option<CreateTagNotificationBody>,
        ///Format for notifications
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_format: ::std::option::Option<CreateTagNotificationFormat>,
        ///Whether notifications are muted
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_muted: ::std::option::Option<bool>,
        ///Include screenshot in notifications (if supported by notification
        /// URL)
        #[serde(default)]
        pub notification_screenshot: bool,
        ///Custom notification title
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_title: ::std::option::Option<CreateTagNotificationTitle>,
        ///Notification URLs for this web page change monitor (watch). Maximum
        /// 100 URLs.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub notification_urls: ::std::vec::Vec<CreateTagNotificationUrlsItem>,
        ///Whether this tag's settings override watch settings for all watches
        /// in this tag/group.
        /// - true: Tag settings override watch settings
        /// - false: Tag settings do not override (watches use their own
        ///   settings)
        /// - null: Not decided yet / inherit default behavior
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub overrides_watch: ::std::option::Option<bool>,
        ///Whether the web page change monitor (watch) is paused
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub paused: ::std::option::Option<bool>,
        ///Minimum price change percentage to trigger notification
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_change_threshold_percent: ::std::option::Option<f64>,
        ///Optional processor mode to use for change detection. Defaults to
        /// `text_json_diff` if not specified.
        #[serde(default = "defaults::create_tag_processor")]
        pub processor: CreateTagProcessor,
        ///Proxy configuration
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxy: ::std::option::Option<CreateTagProxy>,
        ///Remove duplicate lines from content
        #[serde(default)]
        pub remove_duplicate_lines: bool,
        ///Sort lines alphabetically before comparison
        #[serde(default)]
        pub sort_text_alphabetically: bool,
        ///Remove lines matching ignore patterns
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub strip_ignored_lines: ::std::option::Option<bool>,
        ///CSS/XPath selectors to remove content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub subtractive_selectors: ::std::vec::Vec<CreateTagSubtractiveSelectorsItem>,
        ///Tag UUID to associate with this web page change monitor (watch)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tag: ::std::option::Option<CreateTagTag>,
        ///Array of tag UUIDs
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///Text that should NOT be present (triggers alert if found)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub text_should_not_be_present: ::std::vec::Vec<CreateTagTextShouldNotBePresentItem>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_between_check: ::std::option::Option<CreateTagTimeBetweenCheck>,
        ///Whether to use global settings for time between checks - defaults to
        /// true if not set
        #[serde(default = "defaults::default_bool::<true>")]
        pub time_between_check_use_default: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_schedule_limit: ::std::option::Option<CreateTagTimeScheduleLimit>,
        ///Custom title for the web page change monitor (watch), not to be
        /// confused with page_title
        pub title: ::std::option::Option<CreateTagTitle>,
        ///Whether to track JSON-LD price data
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub track_ldjson_price_data: ::std::option::Option<bool>,
        ///Text/regex patterns that must be present to trigger a change
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub trigger_text: ::std::vec::Vec<CreateTagTriggerTextItem>,
        ///Strip leading/trailing whitespace from text
        #[serde(default)]
        pub trim_text_whitespace: bool,
        ///URL to monitor for changes
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub url: ::std::option::Option<::std::string::String>,
        ///Automatically apply this tag to any watch whose URL matches this
        /// pattern. Supports fnmatch wildcards (* and ?): e.g.
        /// *://example.com/* or github.com/myorg. Plain strings are
        /// matched as case-insensitive substrings. Leave empty to
        /// disable auto-matching.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub url_match_pattern: ::std::option::Option<::std::string::String>,
        ///Display page title in watch list (null = use system default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub use_page_title_in_list: ::std::option::Option<bool>,
        ///Unique identifier
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub uuid: ::std::option::Option<::uuid::Uuid>,
        ///Delay in seconds for webdriver
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_delay: ::std::option::Option<i64>,
        ///JavaScript code to execute
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_js_execute_code: ::std::option::Option<CreateTagWebdriverJsExecuteCode>,
    }

    ///HTTP request body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP request body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagBody(::std::string::String);
    impl ::std::ops::Deref for CreateTagBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagBody> for ::std::string::String {
        fn from(value: CreateTagBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateTagBrowserStepsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "operation",
    ///    "optional_value",
    ///    "selector"
    ///  ],
    ///  "properties": {
    ///    "operation": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "optional_value": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "selector": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct CreateTagBrowserStepsItem {
        pub operation: ::std::option::Option<CreateTagBrowserStepsItemOperation>,
        pub optional_value: ::std::option::Option<CreateTagBrowserStepsItemOptionalValue>,
        pub selector: ::std::option::Option<CreateTagBrowserStepsItemSelector>,
    }

    ///`CreateTagBrowserStepsItemOperation`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagBrowserStepsItemOperation(::std::string::String);
    impl ::std::ops::Deref for CreateTagBrowserStepsItemOperation {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagBrowserStepsItemOperation> for ::std::string::String {
        fn from(value: CreateTagBrowserStepsItemOperation) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagBrowserStepsItemOperation {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagBrowserStepsItemOperation {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateTagBrowserStepsItemOptionalValue`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagBrowserStepsItemOptionalValue(::std::string::String);
    impl ::std::ops::Deref for CreateTagBrowserStepsItemOptionalValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagBrowserStepsItemOptionalValue> for ::std::string::String {
        fn from(value: CreateTagBrowserStepsItemOptionalValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagBrowserStepsItemOptionalValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagBrowserStepsItemOptionalValue {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateTagBrowserStepsItemSelector`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagBrowserStepsItemSelector(::std::string::String);
    impl ::std::ops::Deref for CreateTagBrowserStepsItemSelector {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagBrowserStepsItemSelector> for ::std::string::String {
        fn from(value: CreateTagBrowserStepsItemSelector) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagBrowserStepsItemSelector {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagBrowserStepsItemSelector {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateTagConditionsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "field",
    ///    "operator",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "field": {
    ///      "description": "Field to check (e.g., 'page_filtered_text',
    /// 'page_title')",
    ///      "type": "string"
    ///    },
    ///    "operator": {
    ///      "description": "Comparison operator (e.g., 'contains_regex',
    /// 'equals', 'not_equals')",
    ///      "type": "string"
    ///    },
    ///    "value": {
    ///      "description": "Value to compare against",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateTagConditionsItem {
        ///Field to check (e.g., 'page_filtered_text', 'page_title')
        pub field: ::std::string::String,
        ///Comparison operator (e.g., 'contains_regex', 'equals', 'not_equals')
        pub operator: ::std::string::String,
        ///Value to compare against
        pub value: ::std::string::String,
    }

    ///Logic operator - ALL (match all conditions) or ANY (match any condition)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Logic operator - ALL (match all conditions) or ANY
    /// (match any condition)",
    ///  "default": "ALL",
    ///  "type": "string",
    ///  "enum": [
    ///    "ALL",
    ///    "ANY"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum CreateTagConditionsMatchLogic {
        #[serde(rename = "ALL")]
        All,
        #[serde(rename = "ANY")]
        Any,
    }

    impl ::std::fmt::Display for CreateTagConditionsMatchLogic {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::All => f.write_str("ALL"),
                Self::Any => f.write_str("ANY"),
            }
        }
    }

    impl ::std::str::FromStr for CreateTagConditionsMatchLogic {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "ALL" => Ok(Self::All),
                "ANY" => Ok(Self::Any),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for CreateTagConditionsMatchLogic {
        fn default() -> Self {
            CreateTagConditionsMatchLogic::All
        }
    }

    ///`CreateTagExtractLinesContainingItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagExtractLinesContainingItem(::std::string::String);
    impl ::std::ops::Deref for CreateTagExtractLinesContainingItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagExtractLinesContainingItem> for ::std::string::String {
        fn from(value: CreateTagExtractLinesContainingItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagExtractLinesContainingItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagExtractLinesContainingItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateTagExtractTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagExtractTextItem(::std::string::String);
    impl ::std::ops::Deref for CreateTagExtractTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagExtractTextItem> for ::std::string::String {
        fn from(value: CreateTagExtractTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagExtractTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagExtractTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Backend to use for fetching content. Common values:
    /// - `system` (default) - Use the system-wide default fetcher
    /// - `html_requests` - Fast requests-based fetcher
    /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
    /// - `extra_browser_*` - Custom browser configurations (if configured)
    /// - Plugin-provided fetchers (if installed)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Backend to use for fetching content. Common values:\n-
    /// `system` (default) - Use the system-wide default fetcher\n-
    /// `html_requests` - Fast requests-based fetcher\n- `html_webdriver` -
    /// Browser-based fetcher (Playwright/Puppeteer)\n- `extra_browser_*` -
    /// Custom browser configurations (if configured)\n- Plugin-provided
    /// fetchers (if installed)\n",
    ///  "default": "system",
    ///  "type": "string",
    ///  "pattern": "^(system|html_requests|html_webdriver|extra_browser_.+)$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagFetchBackend(::std::string::String);
    impl ::std::ops::Deref for CreateTagFetchBackend {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagFetchBackend> for ::std::string::String {
        fn from(value: CreateTagFetchBackend) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for CreateTagFetchBackend {
        fn default() -> Self {
            CreateTagFetchBackend("system".to_string())
        }
    }

    impl ::std::str::FromStr for CreateTagFetchBackend {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new(
                        "^(system|html_requests|html_webdriver|extra_browser_.+)$",
                    )
                    .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^(system|html_requests|html_webdriver|extra_browser_.+)$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagFetchBackend {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateTagIgnoreTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagIgnoreTextItem(::std::string::String);
    impl ::std::ops::Deref for CreateTagIgnoreTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagIgnoreTextItem> for ::std::string::String {
        fn from(value: CreateTagIgnoreTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagIgnoreTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagIgnoreTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateTagIncludeFiltersItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagIncludeFiltersItem(::std::string::String);
    impl ::std::ops::Deref for CreateTagIncludeFiltersItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagIncludeFiltersItem> for ::std::string::String {
        fn from(value: CreateTagIncludeFiltersItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagIncludeFiltersItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagIncludeFiltersItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///HTTP method to use
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP method to use",
    ///  "type": "string",
    ///  "enum": [
    ///    "GET",
    ///    "POST",
    ///    "DELETE",
    ///    "PUT"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum CreateTagMethod {
        #[serde(rename = "GET")]
        Get,
        #[serde(rename = "POST")]
        Post,
        #[serde(rename = "DELETE")]
        Delete,
        #[serde(rename = "PUT")]
        Put,
    }

    impl ::std::fmt::Display for CreateTagMethod {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Get => f.write_str("GET"),
                Self::Post => f.write_str("POST"),
                Self::Delete => f.write_str("DELETE"),
                Self::Put => f.write_str("PUT"),
            }
        }
    }

    impl ::std::str::FromStr for CreateTagMethod {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "GET" => Ok(Self::Get),
                "POST" => Ok(Self::Post),
                "DELETE" => Ok(Self::Delete),
                "PUT" => Ok(Self::Put),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagMethod {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagNotificationBody(::std::string::String);
    impl ::std::ops::Deref for CreateTagNotificationBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagNotificationBody> for ::std::string::String {
        fn from(value: CreateTagNotificationBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagNotificationBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagNotificationBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Format for notifications
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Format for notifications",
    ///  "type": "string",
    ///  "enum": [
    ///    "text",
    ///    "html",
    ///    "htmlcolor",
    ///    "markdown",
    ///    "System default"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum CreateTagNotificationFormat {
        #[serde(rename = "text")]
        Text,
        #[serde(rename = "html")]
        Html,
        #[serde(rename = "htmlcolor")]
        Htmlcolor,
        #[serde(rename = "markdown")]
        Markdown,
        #[serde(rename = "System default")]
        SystemDefault,
    }

    impl ::std::fmt::Display for CreateTagNotificationFormat {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Text => f.write_str("text"),
                Self::Html => f.write_str("html"),
                Self::Htmlcolor => f.write_str("htmlcolor"),
                Self::Markdown => f.write_str("markdown"),
                Self::SystemDefault => f.write_str("System default"),
            }
        }
    }

    impl ::std::str::FromStr for CreateTagNotificationFormat {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "text" => Ok(Self::Text),
                "html" => Ok(Self::Html),
                "htmlcolor" => Ok(Self::Htmlcolor),
                "markdown" => Ok(Self::Markdown),
                "System default" => Ok(Self::SystemDefault),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagNotificationTitle(::std::string::String);
    impl ::std::ops::Deref for CreateTagNotificationTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagNotificationTitle> for ::std::string::String {
        fn from(value: CreateTagNotificationTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagNotificationTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagNotificationTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateTagNotificationUrlsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 1000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagNotificationUrlsItem(::std::string::String);
    impl ::std::ops::Deref for CreateTagNotificationUrlsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagNotificationUrlsItem> for ::std::string::String {
        fn from(value: CreateTagNotificationUrlsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagNotificationUrlsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 1000usize {
                return Err("longer than 1000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagNotificationUrlsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Optional processor mode to use for change detection. Defaults to
    /// `text_json_diff` if not specified.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Optional processor mode to use for change detection.
    /// Defaults to `text_json_diff` if not specified.",
    ///  "default": "text_json_diff",
    ///  "type": "string",
    ///  "enum": [
    ///    "restock_diff",
    ///    "text_json_diff"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum CreateTagProcessor {
        #[serde(rename = "restock_diff")]
        RestockDiff,
        #[serde(rename = "text_json_diff")]
        TextJsonDiff,
    }

    impl ::std::fmt::Display for CreateTagProcessor {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::RestockDiff => f.write_str("restock_diff"),
                Self::TextJsonDiff => f.write_str("text_json_diff"),
            }
        }
    }

    impl ::std::str::FromStr for CreateTagProcessor {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "restock_diff" => Ok(Self::RestockDiff),
                "text_json_diff" => Ok(Self::TextJsonDiff),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagProcessor {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for CreateTagProcessor {
        fn default() -> Self {
            CreateTagProcessor::TextJsonDiff
        }
    }

    ///Proxy configuration
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Proxy configuration",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagProxy(::std::string::String);
    impl ::std::ops::Deref for CreateTagProxy {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagProxy> for ::std::string::String {
        fn from(value: CreateTagProxy) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagProxy {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagProxy {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagProxy {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateTagResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "uuid": {
    ///      "description": "UUID of the created tag",
    ///      "type": "string",
    ///      "format": "uuid"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateTagResponse {
        ///UUID of the created tag
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub uuid: ::std::option::Option<::uuid::Uuid>,
    }

    impl ::std::default::Default for CreateTagResponse {
        fn default() -> Self {
            Self {
                uuid: Default::default(),
            }
        }
    }

    ///`CreateTagSubtractiveSelectorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagSubtractiveSelectorsItem(::std::string::String);
    impl ::std::ops::Deref for CreateTagSubtractiveSelectorsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagSubtractiveSelectorsItem> for ::std::string::String {
        fn from(value: CreateTagSubtractiveSelectorsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagSubtractiveSelectorsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagSubtractiveSelectorsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Tag UUID to associate with this web page change monitor (watch)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Tag UUID to associate with this web page change monitor
    /// (watch)",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagTag(::std::string::String);
    impl ::std::ops::Deref for CreateTagTag {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagTag> for ::std::string::String {
        fn from(value: CreateTagTag) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagTag {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagTag {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagTag {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateTagTextShouldNotBePresentItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagTextShouldNotBePresentItem(::std::string::String);
    impl ::std::ops::Deref for CreateTagTextShouldNotBePresentItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagTextShouldNotBePresentItem> for ::std::string::String {
        fn from(value: CreateTagTextShouldNotBePresentItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagTextShouldNotBePresentItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagTextShouldNotBePresentItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Time intervals between checks. All fields must be non-negative. At least
    /// one non-zero value required when not using default settings.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Time intervals between checks. All fields must be
    /// non-negative. At least one non-zero value required when not using
    /// default settings.",
    ///  "type": "object",
    ///  "properties": {
    ///    "days": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 365000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "hours": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 8760000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "minutes": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 525600000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "seconds": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 31536000000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "weeks": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 52000.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateTagTimeBetweenCheck {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub days: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub hours: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub minutes: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub seconds: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub weeks: ::std::option::Option<i64>,
    }

    impl ::std::default::Default for CreateTagTimeBetweenCheck {
        fn default() -> Self {
            Self {
                days: Default::default(),
                hours: Default::default(),
                minutes: Default::default(),
                seconds: Default::default(),
                weeks: Default::default(),
            }
        }
    }

    ///Weekly schedule limiting when checks can run
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Weekly schedule limiting when checks can run",
    ///  "type": "object",
    ///  "properties": {
    ///    "enabled": {
    ///      "default": false,
    ///      "type": "boolean"
    ///    },
    ///    "friday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "monday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "saturday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "sunday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "thursday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "tuesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "wednesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateTagTimeScheduleLimit {
        #[serde(default)]
        pub enabled: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub friday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub monday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub saturday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub sunday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub thursday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tuesday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub wednesday: ::std::option::Option<DaySchedule>,
    }

    impl ::std::default::Default for CreateTagTimeScheduleLimit {
        fn default() -> Self {
            Self {
                enabled: Default::default(),
                friday: Default::default(),
                monday: Default::default(),
                saturday: Default::default(),
                sunday: Default::default(),
                thursday: Default::default(),
                tuesday: Default::default(),
                wednesday: Default::default(),
            }
        }
    }

    ///Custom title for the web page change monitor (watch), not to be confused
    /// with page_title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom title for the web page change monitor (watch),
    /// not to be confused with page_title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagTitle(::std::string::String);
    impl ::std::ops::Deref for CreateTagTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagTitle> for ::std::string::String {
        fn from(value: CreateTagTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateTagTriggerTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagTriggerTextItem(::std::string::String);
    impl ::std::ops::Deref for CreateTagTriggerTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagTriggerTextItem> for ::std::string::String {
        fn from(value: CreateTagTriggerTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagTriggerTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagTriggerTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///JavaScript code to execute
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "JavaScript code to execute",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateTagWebdriverJsExecuteCode(::std::string::String);
    impl ::std::ops::Deref for CreateTagWebdriverJsExecuteCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateTagWebdriverJsExecuteCode> for ::std::string::String {
        fn from(value: CreateTagWebdriverJsExecuteCode) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateTagWebdriverJsExecuteCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateTagWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateTagWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateTagWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateTagWebdriverJsExecuteCode {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatch`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/WatchBase"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "url"
    ///      ]
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateWatch {
        ///HTTP request body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub body: ::std::option::Option<CreateWatchBody>,
        ///Browser automation steps. Maximum 100 steps allowed.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub browser_steps: ::std::vec::Vec<CreateWatchBrowserStepsItem>,
        ///Compare against all history for unique lines
        #[serde(default)]
        pub check_unique_lines: bool,
        ///Array of condition rules for change detection logic (empty array
        /// when not set)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub conditions: ::std::vec::Vec<CreateWatchConditionsItem>,
        ///Logic operator - ALL (match all conditions) or ANY (match any
        /// condition)
        #[serde(default = "defaults::create_watch_conditions_match_logic")]
        pub conditions_match_logic: CreateWatchConditionsMatchLogic,
        ///Unix timestamp of creation
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub date_created: ::std::option::Option<i64>,
        ///Keep only lines containing these substrings (plain text,
        /// case-insensitive) — simpler alternative to regex
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_lines_containing: ::std::vec::Vec<CreateWatchExtractLinesContainingItem>,
        ///Regex patterns to extract specific text after filtering
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_text: ::std::vec::Vec<CreateWatchExtractTextItem>,
        ///Backend to use for fetching content. Common values:
        /// - `system` (default) - Use the system-wide default fetcher
        /// - `html_requests` - Fast requests-based fetcher
        /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
        /// - `extra_browser_*` - Custom browser configurations (if configured)
        /// - Plugin-provided fetchers (if installed)
        #[serde(default = "defaults::create_watch_fetch_backend")]
        pub fetch_backend: CreateWatchFetchBackend,
        ///Send notification when filters fail to match content
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_failure_notification_send: bool,
        ///Include added text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_added: bool,
        ///Include removed text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_removed: bool,
        ///Include replaced text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_replaced: bool,
        ///Monitor and track price changes (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub follow_price_changes: bool,
        ///Whether page has LD-JSON price data (auto-detected)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub has_ldjson_price_data: ::std::option::Option<bool>,
        ///HTTP headers to include in requests
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub headers: ::std::collections::HashMap<::std::string::String, ::std::string::String>,
        ///Maximum number of history snapshots to keep (null = use system
        /// default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub history_snapshot_max_length: ::std::option::Option<::std::num::NonZeroU64>,
        ///Ignore HTTP status code errors (boolean or null)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ignore_status_codes: ::std::option::Option<bool>,
        ///Text patterns to ignore in change detection
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub ignore_text: ::std::vec::Vec<CreateWatchIgnoreTextItem>,
        ///Only trigger on in-stock transitions (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub in_stock_only: bool,
        ///CSS/XPath selectors to extract specific content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub include_filters: ::std::vec::Vec<CreateWatchIncludeFiltersItem>,
        ///Internal cache of AI evaluation results keyed by (intent, diff) hash
        /// (auto-managed).
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub llm_evaluation_cache: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        ///Number of tokens consumed by the AI on the most recent check.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_last_tokens_used: ::std::option::Option<i64>,
        ///CSS selector derived by the AI to narrow content scope before
        /// evaluation (auto-managed, do not set manually).
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_prefilter: ::std::option::Option<::std::string::String>,
        ///Total tokens consumed by the AI across all checks for this watch.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_tokens_used_cumulative: ::std::option::Option<i64>,
        ///HTTP method to use
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub method: ::std::option::Option<CreateWatchMethod>,
        ///Custom notification body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_body: ::std::option::Option<CreateWatchNotificationBody>,
        ///Format for notifications
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_format: ::std::option::Option<CreateWatchNotificationFormat>,
        ///Whether notifications are muted
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_muted: ::std::option::Option<bool>,
        ///Include screenshot in notifications (if supported by notification
        /// URL)
        #[serde(default)]
        pub notification_screenshot: bool,
        ///Custom notification title
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_title: ::std::option::Option<CreateWatchNotificationTitle>,
        ///Notification URLs for this web page change monitor (watch). Maximum
        /// 100 URLs.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub notification_urls: ::std::vec::Vec<CreateWatchNotificationUrlsItem>,
        ///Whether the web page change monitor (watch) is paused
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub paused: ::std::option::Option<bool>,
        ///Minimum price change percentage to trigger notification
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_change_threshold_percent: ::std::option::Option<f64>,
        ///Optional processor mode to use for change detection. Defaults to
        /// `text_json_diff` if not specified.
        #[serde(default = "defaults::create_watch_processor")]
        pub processor: CreateWatchProcessor,
        ///Proxy configuration
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxy: ::std::option::Option<CreateWatchProxy>,
        ///Remove duplicate lines from content
        #[serde(default)]
        pub remove_duplicate_lines: bool,
        ///Sort lines alphabetically before comparison
        #[serde(default)]
        pub sort_text_alphabetically: bool,
        ///Remove lines matching ignore patterns
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub strip_ignored_lines: ::std::option::Option<bool>,
        ///CSS/XPath selectors to remove content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub subtractive_selectors: ::std::vec::Vec<CreateWatchSubtractiveSelectorsItem>,
        ///Tag UUID to associate with this web page change monitor (watch)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tag: ::std::option::Option<CreateWatchTag>,
        ///Array of tag UUIDs
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///Text that should NOT be present (triggers alert if found)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub text_should_not_be_present: ::std::vec::Vec<CreateWatchTextShouldNotBePresentItem>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_between_check: ::std::option::Option<CreateWatchTimeBetweenCheck>,
        ///Whether to use global settings for time between checks - defaults to
        /// true if not set
        #[serde(default = "defaults::default_bool::<true>")]
        pub time_between_check_use_default: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_schedule_limit: ::std::option::Option<CreateWatchTimeScheduleLimit>,
        ///Custom title for the web page change monitor (watch), not to be
        /// confused with page_title
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub title: ::std::option::Option<CreateWatchTitle>,
        ///Whether to track JSON-LD price data
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub track_ldjson_price_data: ::std::option::Option<bool>,
        ///Text/regex patterns that must be present to trigger a change
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub trigger_text: ::std::vec::Vec<CreateWatchTriggerTextItem>,
        ///Strip leading/trailing whitespace from text
        #[serde(default)]
        pub trim_text_whitespace: bool,
        ///URL to monitor for changes
        pub url: ::std::string::String,
        ///Display page title in watch list (null = use system default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub use_page_title_in_list: ::std::option::Option<bool>,
        ///Unique identifier
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub uuid: ::std::option::Option<::uuid::Uuid>,
        ///Delay in seconds for webdriver
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_delay: ::std::option::Option<i64>,
        ///JavaScript code to execute
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_js_execute_code: ::std::option::Option<CreateWatchWebdriverJsExecuteCode>,
    }

    ///HTTP request body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP request body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchBody(::std::string::String);
    impl ::std::ops::Deref for CreateWatchBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchBody> for ::std::string::String {
        fn from(value: CreateWatchBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatchBrowserStepsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "operation",
    ///    "optional_value",
    ///    "selector"
    ///  ],
    ///  "properties": {
    ///    "operation": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "optional_value": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "selector": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct CreateWatchBrowserStepsItem {
        pub operation: ::std::option::Option<CreateWatchBrowserStepsItemOperation>,
        pub optional_value: ::std::option::Option<CreateWatchBrowserStepsItemOptionalValue>,
        pub selector: ::std::option::Option<CreateWatchBrowserStepsItemSelector>,
    }

    ///`CreateWatchBrowserStepsItemOperation`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchBrowserStepsItemOperation(::std::string::String);
    impl ::std::ops::Deref for CreateWatchBrowserStepsItemOperation {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchBrowserStepsItemOperation> for ::std::string::String {
        fn from(value: CreateWatchBrowserStepsItemOperation) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchBrowserStepsItemOperation {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchBrowserStepsItemOperation {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatchBrowserStepsItemOptionalValue`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchBrowserStepsItemOptionalValue(::std::string::String);
    impl ::std::ops::Deref for CreateWatchBrowserStepsItemOptionalValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchBrowserStepsItemOptionalValue> for ::std::string::String {
        fn from(value: CreateWatchBrowserStepsItemOptionalValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchBrowserStepsItemOptionalValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchBrowserStepsItemOptionalValue {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatchBrowserStepsItemSelector`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchBrowserStepsItemSelector(::std::string::String);
    impl ::std::ops::Deref for CreateWatchBrowserStepsItemSelector {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchBrowserStepsItemSelector> for ::std::string::String {
        fn from(value: CreateWatchBrowserStepsItemSelector) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchBrowserStepsItemSelector {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchBrowserStepsItemSelector {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatchConditionsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "field",
    ///    "operator",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "field": {
    ///      "description": "Field to check (e.g., 'page_filtered_text',
    /// 'page_title')",
    ///      "type": "string"
    ///    },
    ///    "operator": {
    ///      "description": "Comparison operator (e.g., 'contains_regex',
    /// 'equals', 'not_equals')",
    ///      "type": "string"
    ///    },
    ///    "value": {
    ///      "description": "Value to compare against",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateWatchConditionsItem {
        ///Field to check (e.g., 'page_filtered_text', 'page_title')
        pub field: ::std::string::String,
        ///Comparison operator (e.g., 'contains_regex', 'equals', 'not_equals')
        pub operator: ::std::string::String,
        ///Value to compare against
        pub value: ::std::string::String,
    }

    ///Logic operator - ALL (match all conditions) or ANY (match any condition)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Logic operator - ALL (match all conditions) or ANY
    /// (match any condition)",
    ///  "default": "ALL",
    ///  "type": "string",
    ///  "enum": [
    ///    "ALL",
    ///    "ANY"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum CreateWatchConditionsMatchLogic {
        #[serde(rename = "ALL")]
        All,
        #[serde(rename = "ANY")]
        Any,
    }

    impl ::std::fmt::Display for CreateWatchConditionsMatchLogic {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::All => f.write_str("ALL"),
                Self::Any => f.write_str("ANY"),
            }
        }
    }

    impl ::std::str::FromStr for CreateWatchConditionsMatchLogic {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "ALL" => Ok(Self::All),
                "ANY" => Ok(Self::Any),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for CreateWatchConditionsMatchLogic {
        fn default() -> Self {
            CreateWatchConditionsMatchLogic::All
        }
    }

    ///`CreateWatchExtractLinesContainingItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchExtractLinesContainingItem(::std::string::String);
    impl ::std::ops::Deref for CreateWatchExtractLinesContainingItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchExtractLinesContainingItem> for ::std::string::String {
        fn from(value: CreateWatchExtractLinesContainingItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchExtractLinesContainingItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchExtractLinesContainingItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatchExtractTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchExtractTextItem(::std::string::String);
    impl ::std::ops::Deref for CreateWatchExtractTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchExtractTextItem> for ::std::string::String {
        fn from(value: CreateWatchExtractTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchExtractTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchExtractTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Backend to use for fetching content. Common values:
    /// - `system` (default) - Use the system-wide default fetcher
    /// - `html_requests` - Fast requests-based fetcher
    /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
    /// - `extra_browser_*` - Custom browser configurations (if configured)
    /// - Plugin-provided fetchers (if installed)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Backend to use for fetching content. Common values:\n-
    /// `system` (default) - Use the system-wide default fetcher\n-
    /// `html_requests` - Fast requests-based fetcher\n- `html_webdriver` -
    /// Browser-based fetcher (Playwright/Puppeteer)\n- `extra_browser_*` -
    /// Custom browser configurations (if configured)\n- Plugin-provided
    /// fetchers (if installed)\n",
    ///  "default": "system",
    ///  "type": "string",
    ///  "pattern": "^(system|html_requests|html_webdriver|extra_browser_.+)$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchFetchBackend(::std::string::String);
    impl ::std::ops::Deref for CreateWatchFetchBackend {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchFetchBackend> for ::std::string::String {
        fn from(value: CreateWatchFetchBackend) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for CreateWatchFetchBackend {
        fn default() -> Self {
            CreateWatchFetchBackend("system".to_string())
        }
    }

    impl ::std::str::FromStr for CreateWatchFetchBackend {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new(
                        "^(system|html_requests|html_webdriver|extra_browser_.+)$",
                    )
                    .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^(system|html_requests|html_webdriver|extra_browser_.+)$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchFetchBackend {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatchIgnoreTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchIgnoreTextItem(::std::string::String);
    impl ::std::ops::Deref for CreateWatchIgnoreTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchIgnoreTextItem> for ::std::string::String {
        fn from(value: CreateWatchIgnoreTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchIgnoreTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchIgnoreTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatchIncludeFiltersItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchIncludeFiltersItem(::std::string::String);
    impl ::std::ops::Deref for CreateWatchIncludeFiltersItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchIncludeFiltersItem> for ::std::string::String {
        fn from(value: CreateWatchIncludeFiltersItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchIncludeFiltersItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchIncludeFiltersItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///HTTP method to use
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP method to use",
    ///  "type": "string",
    ///  "enum": [
    ///    "GET",
    ///    "POST",
    ///    "DELETE",
    ///    "PUT"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum CreateWatchMethod {
        #[serde(rename = "GET")]
        Get,
        #[serde(rename = "POST")]
        Post,
        #[serde(rename = "DELETE")]
        Delete,
        #[serde(rename = "PUT")]
        Put,
    }

    impl ::std::fmt::Display for CreateWatchMethod {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Get => f.write_str("GET"),
                Self::Post => f.write_str("POST"),
                Self::Delete => f.write_str("DELETE"),
                Self::Put => f.write_str("PUT"),
            }
        }
    }

    impl ::std::str::FromStr for CreateWatchMethod {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "GET" => Ok(Self::Get),
                "POST" => Ok(Self::Post),
                "DELETE" => Ok(Self::Delete),
                "PUT" => Ok(Self::Put),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchMethod {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchNotificationBody(::std::string::String);
    impl ::std::ops::Deref for CreateWatchNotificationBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchNotificationBody> for ::std::string::String {
        fn from(value: CreateWatchNotificationBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchNotificationBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchNotificationBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Format for notifications
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Format for notifications",
    ///  "type": "string",
    ///  "enum": [
    ///    "text",
    ///    "html",
    ///    "htmlcolor",
    ///    "markdown",
    ///    "System default"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum CreateWatchNotificationFormat {
        #[serde(rename = "text")]
        Text,
        #[serde(rename = "html")]
        Html,
        #[serde(rename = "htmlcolor")]
        Htmlcolor,
        #[serde(rename = "markdown")]
        Markdown,
        #[serde(rename = "System default")]
        SystemDefault,
    }

    impl ::std::fmt::Display for CreateWatchNotificationFormat {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Text => f.write_str("text"),
                Self::Html => f.write_str("html"),
                Self::Htmlcolor => f.write_str("htmlcolor"),
                Self::Markdown => f.write_str("markdown"),
                Self::SystemDefault => f.write_str("System default"),
            }
        }
    }

    impl ::std::str::FromStr for CreateWatchNotificationFormat {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "text" => Ok(Self::Text),
                "html" => Ok(Self::Html),
                "htmlcolor" => Ok(Self::Htmlcolor),
                "markdown" => Ok(Self::Markdown),
                "System default" => Ok(Self::SystemDefault),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchNotificationTitle(::std::string::String);
    impl ::std::ops::Deref for CreateWatchNotificationTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchNotificationTitle> for ::std::string::String {
        fn from(value: CreateWatchNotificationTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchNotificationTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchNotificationTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatchNotificationUrlsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 1000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchNotificationUrlsItem(::std::string::String);
    impl ::std::ops::Deref for CreateWatchNotificationUrlsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchNotificationUrlsItem> for ::std::string::String {
        fn from(value: CreateWatchNotificationUrlsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchNotificationUrlsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 1000usize {
                return Err("longer than 1000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchNotificationUrlsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Optional processor mode to use for change detection. Defaults to
    /// `text_json_diff` if not specified.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Optional processor mode to use for change detection.
    /// Defaults to `text_json_diff` if not specified.",
    ///  "default": "text_json_diff",
    ///  "type": "string",
    ///  "enum": [
    ///    "restock_diff",
    ///    "text_json_diff"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum CreateWatchProcessor {
        #[serde(rename = "restock_diff")]
        RestockDiff,
        #[serde(rename = "text_json_diff")]
        TextJsonDiff,
    }

    impl ::std::fmt::Display for CreateWatchProcessor {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::RestockDiff => f.write_str("restock_diff"),
                Self::TextJsonDiff => f.write_str("text_json_diff"),
            }
        }
    }

    impl ::std::str::FromStr for CreateWatchProcessor {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "restock_diff" => Ok(Self::RestockDiff),
                "text_json_diff" => Ok(Self::TextJsonDiff),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchProcessor {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for CreateWatchProcessor {
        fn default() -> Self {
            CreateWatchProcessor::TextJsonDiff
        }
    }

    ///Proxy configuration
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Proxy configuration",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchProxy(::std::string::String);
    impl ::std::ops::Deref for CreateWatchProxy {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchProxy> for ::std::string::String {
        fn from(value: CreateWatchProxy) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchProxy {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchProxy {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchProxy {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatchSubtractiveSelectorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchSubtractiveSelectorsItem(::std::string::String);
    impl ::std::ops::Deref for CreateWatchSubtractiveSelectorsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchSubtractiveSelectorsItem> for ::std::string::String {
        fn from(value: CreateWatchSubtractiveSelectorsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchSubtractiveSelectorsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchSubtractiveSelectorsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Tag UUID to associate with this web page change monitor (watch)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Tag UUID to associate with this web page change monitor
    /// (watch)",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchTag(::std::string::String);
    impl ::std::ops::Deref for CreateWatchTag {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchTag> for ::std::string::String {
        fn from(value: CreateWatchTag) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchTag {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchTag {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchTag {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatchTextShouldNotBePresentItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchTextShouldNotBePresentItem(::std::string::String);
    impl ::std::ops::Deref for CreateWatchTextShouldNotBePresentItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchTextShouldNotBePresentItem> for ::std::string::String {
        fn from(value: CreateWatchTextShouldNotBePresentItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchTextShouldNotBePresentItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchTextShouldNotBePresentItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Time intervals between checks. All fields must be non-negative. At least
    /// one non-zero value required when not using default settings.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Time intervals between checks. All fields must be
    /// non-negative. At least one non-zero value required when not using
    /// default settings.",
    ///  "type": "object",
    ///  "properties": {
    ///    "days": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 365000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "hours": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 8760000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "minutes": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 525600000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "seconds": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 31536000000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "weeks": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 52000.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateWatchTimeBetweenCheck {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub days: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub hours: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub minutes: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub seconds: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub weeks: ::std::option::Option<i64>,
    }

    impl ::std::default::Default for CreateWatchTimeBetweenCheck {
        fn default() -> Self {
            Self {
                days: Default::default(),
                hours: Default::default(),
                minutes: Default::default(),
                seconds: Default::default(),
                weeks: Default::default(),
            }
        }
    }

    ///Weekly schedule limiting when checks can run
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Weekly schedule limiting when checks can run",
    ///  "type": "object",
    ///  "properties": {
    ///    "enabled": {
    ///      "default": false,
    ///      "type": "boolean"
    ///    },
    ///    "friday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "monday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "saturday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "sunday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "thursday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "tuesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "wednesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateWatchTimeScheduleLimit {
        #[serde(default)]
        pub enabled: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub friday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub monday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub saturday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub sunday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub thursday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tuesday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub wednesday: ::std::option::Option<DaySchedule>,
    }

    impl ::std::default::Default for CreateWatchTimeScheduleLimit {
        fn default() -> Self {
            Self {
                enabled: Default::default(),
                friday: Default::default(),
                monday: Default::default(),
                saturday: Default::default(),
                sunday: Default::default(),
                thursday: Default::default(),
                tuesday: Default::default(),
                wednesday: Default::default(),
            }
        }
    }

    ///Custom title for the web page change monitor (watch), not to be confused
    /// with page_title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom title for the web page change monitor (watch),
    /// not to be confused with page_title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchTitle(::std::string::String);
    impl ::std::ops::Deref for CreateWatchTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchTitle> for ::std::string::String {
        fn from(value: CreateWatchTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`CreateWatchTriggerTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchTriggerTextItem(::std::string::String);
    impl ::std::ops::Deref for CreateWatchTriggerTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchTriggerTextItem> for ::std::string::String {
        fn from(value: CreateWatchTriggerTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchTriggerTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchTriggerTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///JavaScript code to execute
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "JavaScript code to execute",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CreateWatchWebdriverJsExecuteCode(::std::string::String);
    impl ::std::ops::Deref for CreateWatchWebdriverJsExecuteCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CreateWatchWebdriverJsExecuteCode> for ::std::string::String {
        fn from(value: CreateWatchWebdriverJsExecuteCode) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CreateWatchWebdriverJsExecuteCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CreateWatchWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CreateWatchWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CreateWatchWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CreateWatchWebdriverJsExecuteCode {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`DaySchedule`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "duration": {
    ///      "type": "object",
    ///      "properties": {
    ///        "hours": {
    ///          "default": "24",
    ///          "type": "string",
    ///          "pattern": "^[0-9]+$"
    ///        },
    ///        "minutes": {
    ///          "default": "00",
    ///          "type": "string",
    ///          "pattern": "^[0-9]+$"
    ///        }
    ///      }
    ///    },
    ///    "enabled": {
    ///      "default": true,
    ///      "type": "boolean"
    ///    },
    ///    "start_time": {
    ///      "description": "Start time in HH:MM format",
    ///      "default": "00:00",
    ///      "type": "string",
    ///      "pattern": "^([0-1]?[0-9]|2[0-3]):[0-5][0-9]$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DaySchedule {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub duration: ::std::option::Option<DayScheduleDuration>,
        #[serde(default = "defaults::default_bool::<true>")]
        pub enabled: bool,
        ///Start time in HH:MM format
        #[serde(default = "defaults::day_schedule_start_time")]
        pub start_time: DayScheduleStartTime,
    }

    impl ::std::default::Default for DaySchedule {
        fn default() -> Self {
            Self {
                duration: Default::default(),
                enabled: defaults::default_bool::<true>(),
                start_time: defaults::day_schedule_start_time(),
            }
        }
    }

    ///`DayScheduleDuration`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "hours": {
    ///      "default": "24",
    ///      "type": "string",
    ///      "pattern": "^[0-9]+$"
    ///    },
    ///    "minutes": {
    ///      "default": "00",
    ///      "type": "string",
    ///      "pattern": "^[0-9]+$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DayScheduleDuration {
        #[serde(default = "defaults::day_schedule_duration_hours")]
        pub hours: DayScheduleDurationHours,
        #[serde(default = "defaults::day_schedule_duration_minutes")]
        pub minutes: DayScheduleDurationMinutes,
    }

    impl ::std::default::Default for DayScheduleDuration {
        fn default() -> Self {
            Self {
                hours: defaults::day_schedule_duration_hours(),
                minutes: defaults::day_schedule_duration_minutes(),
            }
        }
    }

    ///`DayScheduleDurationHours`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "24",
    ///  "type": "string",
    ///  "pattern": "^[0-9]+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct DayScheduleDurationHours(::std::string::String);
    impl ::std::ops::Deref for DayScheduleDurationHours {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DayScheduleDurationHours> for ::std::string::String {
        fn from(value: DayScheduleDurationHours) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for DayScheduleDurationHours {
        fn default() -> Self {
            DayScheduleDurationHours("24".to_string())
        }
    }

    impl ::std::str::FromStr for DayScheduleDurationHours {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[0-9]+$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[0-9]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for DayScheduleDurationHours {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DayScheduleDurationHours {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DayScheduleDurationHours {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for DayScheduleDurationHours {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`DayScheduleDurationMinutes`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "00",
    ///  "type": "string",
    ///  "pattern": "^[0-9]+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct DayScheduleDurationMinutes(::std::string::String);
    impl ::std::ops::Deref for DayScheduleDurationMinutes {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DayScheduleDurationMinutes> for ::std::string::String {
        fn from(value: DayScheduleDurationMinutes) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for DayScheduleDurationMinutes {
        fn default() -> Self {
            DayScheduleDurationMinutes("00".to_string())
        }
    }

    impl ::std::str::FromStr for DayScheduleDurationMinutes {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[0-9]+$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[0-9]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for DayScheduleDurationMinutes {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DayScheduleDurationMinutes {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DayScheduleDurationMinutes {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for DayScheduleDurationMinutes {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Start time in HH:MM format
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Start time in HH:MM format",
    ///  "default": "00:00",
    ///  "type": "string",
    ///  "pattern": "^([0-1]?[0-9]|2[0-3]):[0-5][0-9]$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct DayScheduleStartTime(::std::string::String);
    impl ::std::ops::Deref for DayScheduleStartTime {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DayScheduleStartTime> for ::std::string::String {
        fn from(value: DayScheduleStartTime) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for DayScheduleStartTime {
        fn default() -> Self {
            DayScheduleStartTime("00:00".to_string())
        }
    }

    impl ::std::str::FromStr for DayScheduleStartTime {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^([0-1]?[0-9]|2[0-3]):[0-5][0-9]$").unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^([0-1]?[0-9]|2[0-3]):[0-5][0-9]$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for DayScheduleStartTime {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DayScheduleStartTime {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DayScheduleStartTime {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for DayScheduleStartTime {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`Error`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "message": {
    ///      "description": "Error message",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct Error {
        ///Error message
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub message: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for Error {
        fn default() -> Self {
            Self {
                message: Default::default(),
            }
        }
    }

    ///`GetTagMuted`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "muted",
    ///    "unmuted"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetTagMuted {
        #[serde(rename = "muted")]
        Muted,
        #[serde(rename = "unmuted")]
        Unmuted,
    }

    impl ::std::fmt::Display for GetTagMuted {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Muted => f.write_str("muted"),
                Self::Unmuted => f.write_str("unmuted"),
            }
        }
    }

    impl ::std::str::FromStr for GetTagMuted {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "muted" => Ok(Self::Muted),
                "unmuted" => Ok(Self::Unmuted),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetTagMuted {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetTagMuted {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetTagMuted {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`GetTagRecheck`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "true"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetTagRecheck {
        #[serde(rename = "true")]
        True,
    }

    impl ::std::fmt::Display for GetTagRecheck {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::True => f.write_str("true"),
            }
        }
    }

    impl ::std::str::FromStr for GetTagRecheck {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "true" => Ok(Self::True),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetTagRecheck {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetTagRecheck {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetTagRecheck {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`GetWatchHistoryDiffAdded`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "true",
    ///  "type": "string",
    ///  "enum": [
    ///    "true",
    ///    "false",
    ///    "1",
    ///    "0",
    ///    "yes",
    ///    "no",
    ///    "on",
    ///    "off"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchHistoryDiffAdded {
        #[serde(rename = "true")]
        True,
        #[serde(rename = "false")]
        False,
        #[serde(rename = "1")]
        X1,
        #[serde(rename = "0")]
        X0,
        #[serde(rename = "yes")]
        Yes,
        #[serde(rename = "no")]
        No,
        #[serde(rename = "on")]
        On,
        #[serde(rename = "off")]
        Off,
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffAdded {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::True => f.write_str("true"),
                Self::False => f.write_str("false"),
                Self::X1 => f.write_str("1"),
                Self::X0 => f.write_str("0"),
                Self::Yes => f.write_str("yes"),
                Self::No => f.write_str("no"),
                Self::On => f.write_str("on"),
                Self::Off => f.write_str("off"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffAdded {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "true" => Ok(Self::True),
                "false" => Ok(Self::False),
                "1" => Ok(Self::X1),
                "0" => Ok(Self::X0),
                "yes" => Ok(Self::Yes),
                "no" => Ok(Self::No),
                "on" => Ok(Self::On),
                "off" => Ok(Self::Off),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffAdded {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffAdded {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffAdded {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for GetWatchHistoryDiffAdded {
        fn default() -> Self {
            GetWatchHistoryDiffAdded::True
        }
    }

    ///`GetWatchHistoryDiffChangesOnly`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "true",
    ///  "type": "string",
    ///  "enum": [
    ///    "true",
    ///    "false",
    ///    "1",
    ///    "0",
    ///    "yes",
    ///    "no",
    ///    "on",
    ///    "off"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchHistoryDiffChangesOnly {
        #[serde(rename = "true")]
        True,
        #[serde(rename = "false")]
        False,
        #[serde(rename = "1")]
        X1,
        #[serde(rename = "0")]
        X0,
        #[serde(rename = "yes")]
        Yes,
        #[serde(rename = "no")]
        No,
        #[serde(rename = "on")]
        On,
        #[serde(rename = "off")]
        Off,
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffChangesOnly {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::True => f.write_str("true"),
                Self::False => f.write_str("false"),
                Self::X1 => f.write_str("1"),
                Self::X0 => f.write_str("0"),
                Self::Yes => f.write_str("yes"),
                Self::No => f.write_str("no"),
                Self::On => f.write_str("on"),
                Self::Off => f.write_str("off"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffChangesOnly {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "true" => Ok(Self::True),
                "false" => Ok(Self::False),
                "1" => Ok(Self::X1),
                "0" => Ok(Self::X0),
                "yes" => Ok(Self::Yes),
                "no" => Ok(Self::No),
                "on" => Ok(Self::On),
                "off" => Ok(Self::Off),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffChangesOnly {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffChangesOnly {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffChangesOnly {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for GetWatchHistoryDiffChangesOnly {
        fn default() -> Self {
            GetWatchHistoryDiffChangesOnly::True
        }
    }

    ///`GetWatchHistoryDiffFormat`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "text",
    ///  "type": "string",
    ///  "enum": [
    ///    "text",
    ///    "html",
    ///    "htmlcolor",
    ///    "markdown"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchHistoryDiffFormat {
        #[serde(rename = "text")]
        Text,
        #[serde(rename = "html")]
        Html,
        #[serde(rename = "htmlcolor")]
        Htmlcolor,
        #[serde(rename = "markdown")]
        Markdown,
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffFormat {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Text => f.write_str("text"),
                Self::Html => f.write_str("html"),
                Self::Htmlcolor => f.write_str("htmlcolor"),
                Self::Markdown => f.write_str("markdown"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffFormat {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "text" => Ok(Self::Text),
                "html" => Ok(Self::Html),
                "htmlcolor" => Ok(Self::Htmlcolor),
                "markdown" => Ok(Self::Markdown),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffFormat {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for GetWatchHistoryDiffFormat {
        fn default() -> Self {
            GetWatchHistoryDiffFormat::Text
        }
    }

    ///`GetWatchHistoryDiffFromTimestamp`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "description": "Unix timestamp of the starting snapshot",
    ///      "type": "integer"
    ///    },
    ///    {
    ///      "description": "Use 'previous' to automatically select the
    /// second-most-recent snapshot",
    ///      "type": "string",
    ///      "enum": [
    ///        "previous"
    ///      ]
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum GetWatchHistoryDiffFromTimestamp {
        Integer(i64),
        String(GetWatchHistoryDiffFromTimestampString),
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffFromTimestamp {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if let Ok(v) = value.parse() {
                Ok(Self::Integer(v))
            } else if let Ok(v) = value.parse() {
                Ok(Self::String(v))
            } else {
                Err("string conversion failed for all variants".into())
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffFromTimestamp {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffFromTimestamp {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffFromTimestamp {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffFromTimestamp {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                Self::Integer(x) => x.fmt(f),
                Self::String(x) => x.fmt(f),
            }
        }
    }

    impl ::std::convert::From<i64> for GetWatchHistoryDiffFromTimestamp {
        fn from(value: i64) -> Self {
            Self::Integer(value)
        }
    }

    impl ::std::convert::From<GetWatchHistoryDiffFromTimestampString>
        for GetWatchHistoryDiffFromTimestamp
    {
        fn from(value: GetWatchHistoryDiffFromTimestampString) -> Self {
            Self::String(value)
        }
    }

    ///Use 'previous' to automatically select the second-most-recent snapshot
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Use 'previous' to automatically select the
    /// second-most-recent snapshot",
    ///  "type": "string",
    ///  "enum": [
    ///    "previous"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchHistoryDiffFromTimestampString {
        #[serde(rename = "previous")]
        Previous,
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffFromTimestampString {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Previous => f.write_str("previous"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffFromTimestampString {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "previous" => Ok(Self::Previous),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffFromTimestampString {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffFromTimestampString {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffFromTimestampString {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`GetWatchHistoryDiffIgnoreWhitespace`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "false",
    ///  "type": "string",
    ///  "enum": [
    ///    "true",
    ///    "false",
    ///    "1",
    ///    "0",
    ///    "yes",
    ///    "no",
    ///    "on",
    ///    "off"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchHistoryDiffIgnoreWhitespace {
        #[serde(rename = "true")]
        True,
        #[serde(rename = "false")]
        False,
        #[serde(rename = "1")]
        X1,
        #[serde(rename = "0")]
        X0,
        #[serde(rename = "yes")]
        Yes,
        #[serde(rename = "no")]
        No,
        #[serde(rename = "on")]
        On,
        #[serde(rename = "off")]
        Off,
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffIgnoreWhitespace {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::True => f.write_str("true"),
                Self::False => f.write_str("false"),
                Self::X1 => f.write_str("1"),
                Self::X0 => f.write_str("0"),
                Self::Yes => f.write_str("yes"),
                Self::No => f.write_str("no"),
                Self::On => f.write_str("on"),
                Self::Off => f.write_str("off"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffIgnoreWhitespace {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "true" => Ok(Self::True),
                "false" => Ok(Self::False),
                "1" => Ok(Self::X1),
                "0" => Ok(Self::X0),
                "yes" => Ok(Self::Yes),
                "no" => Ok(Self::No),
                "on" => Ok(Self::On),
                "off" => Ok(Self::Off),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffIgnoreWhitespace {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffIgnoreWhitespace {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffIgnoreWhitespace {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for GetWatchHistoryDiffIgnoreWhitespace {
        fn default() -> Self {
            GetWatchHistoryDiffIgnoreWhitespace::False
        }
    }

    ///`GetWatchHistoryDiffNoMarkup`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "false",
    ///  "type": "string",
    ///  "enum": [
    ///    "true",
    ///    "false",
    ///    "1",
    ///    "0",
    ///    "yes",
    ///    "no",
    ///    "on",
    ///    "off"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchHistoryDiffNoMarkup {
        #[serde(rename = "true")]
        True,
        #[serde(rename = "false")]
        False,
        #[serde(rename = "1")]
        X1,
        #[serde(rename = "0")]
        X0,
        #[serde(rename = "yes")]
        Yes,
        #[serde(rename = "no")]
        No,
        #[serde(rename = "on")]
        On,
        #[serde(rename = "off")]
        Off,
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffNoMarkup {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::True => f.write_str("true"),
                Self::False => f.write_str("false"),
                Self::X1 => f.write_str("1"),
                Self::X0 => f.write_str("0"),
                Self::Yes => f.write_str("yes"),
                Self::No => f.write_str("no"),
                Self::On => f.write_str("on"),
                Self::Off => f.write_str("off"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffNoMarkup {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "true" => Ok(Self::True),
                "false" => Ok(Self::False),
                "1" => Ok(Self::X1),
                "0" => Ok(Self::X0),
                "yes" => Ok(Self::Yes),
                "no" => Ok(Self::No),
                "on" => Ok(Self::On),
                "off" => Ok(Self::Off),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffNoMarkup {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffNoMarkup {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffNoMarkup {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for GetWatchHistoryDiffNoMarkup {
        fn default() -> Self {
            GetWatchHistoryDiffNoMarkup::False
        }
    }

    ///`GetWatchHistoryDiffRemoved`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "true",
    ///  "type": "string",
    ///  "enum": [
    ///    "true",
    ///    "false",
    ///    "1",
    ///    "0",
    ///    "yes",
    ///    "no",
    ///    "on",
    ///    "off"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchHistoryDiffRemoved {
        #[serde(rename = "true")]
        True,
        #[serde(rename = "false")]
        False,
        #[serde(rename = "1")]
        X1,
        #[serde(rename = "0")]
        X0,
        #[serde(rename = "yes")]
        Yes,
        #[serde(rename = "no")]
        No,
        #[serde(rename = "on")]
        On,
        #[serde(rename = "off")]
        Off,
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffRemoved {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::True => f.write_str("true"),
                Self::False => f.write_str("false"),
                Self::X1 => f.write_str("1"),
                Self::X0 => f.write_str("0"),
                Self::Yes => f.write_str("yes"),
                Self::No => f.write_str("no"),
                Self::On => f.write_str("on"),
                Self::Off => f.write_str("off"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffRemoved {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "true" => Ok(Self::True),
                "false" => Ok(Self::False),
                "1" => Ok(Self::X1),
                "0" => Ok(Self::X0),
                "yes" => Ok(Self::Yes),
                "no" => Ok(Self::No),
                "on" => Ok(Self::On),
                "off" => Ok(Self::Off),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffRemoved {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffRemoved {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffRemoved {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for GetWatchHistoryDiffRemoved {
        fn default() -> Self {
            GetWatchHistoryDiffRemoved::True
        }
    }

    ///`GetWatchHistoryDiffReplaced`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "true",
    ///  "type": "string",
    ///  "enum": [
    ///    "true",
    ///    "false",
    ///    "1",
    ///    "0",
    ///    "yes",
    ///    "no",
    ///    "on",
    ///    "off"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchHistoryDiffReplaced {
        #[serde(rename = "true")]
        True,
        #[serde(rename = "false")]
        False,
        #[serde(rename = "1")]
        X1,
        #[serde(rename = "0")]
        X0,
        #[serde(rename = "yes")]
        Yes,
        #[serde(rename = "no")]
        No,
        #[serde(rename = "on")]
        On,
        #[serde(rename = "off")]
        Off,
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffReplaced {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::True => f.write_str("true"),
                Self::False => f.write_str("false"),
                Self::X1 => f.write_str("1"),
                Self::X0 => f.write_str("0"),
                Self::Yes => f.write_str("yes"),
                Self::No => f.write_str("no"),
                Self::On => f.write_str("on"),
                Self::Off => f.write_str("off"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffReplaced {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "true" => Ok(Self::True),
                "false" => Ok(Self::False),
                "1" => Ok(Self::X1),
                "0" => Ok(Self::X0),
                "yes" => Ok(Self::Yes),
                "no" => Ok(Self::No),
                "on" => Ok(Self::On),
                "off" => Ok(Self::Off),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffReplaced {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffReplaced {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffReplaced {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for GetWatchHistoryDiffReplaced {
        fn default() -> Self {
            GetWatchHistoryDiffReplaced::True
        }
    }

    ///`GetWatchHistoryDiffToTimestamp`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "description": "Unix timestamp of the ending snapshot",
    ///      "type": "integer"
    ///    },
    ///    {
    ///      "description": "Use 'latest' to automatically select the most
    /// recent snapshot",
    ///      "type": "string",
    ///      "enum": [
    ///        "latest"
    ///      ]
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum GetWatchHistoryDiffToTimestamp {
        Integer(i64),
        String(GetWatchHistoryDiffToTimestampString),
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffToTimestamp {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if let Ok(v) = value.parse() {
                Ok(Self::Integer(v))
            } else if let Ok(v) = value.parse() {
                Ok(Self::String(v))
            } else {
                Err("string conversion failed for all variants".into())
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffToTimestamp {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffToTimestamp {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffToTimestamp {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffToTimestamp {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                Self::Integer(x) => x.fmt(f),
                Self::String(x) => x.fmt(f),
            }
        }
    }

    impl ::std::convert::From<i64> for GetWatchHistoryDiffToTimestamp {
        fn from(value: i64) -> Self {
            Self::Integer(value)
        }
    }

    impl ::std::convert::From<GetWatchHistoryDiffToTimestampString> for GetWatchHistoryDiffToTimestamp {
        fn from(value: GetWatchHistoryDiffToTimestampString) -> Self {
            Self::String(value)
        }
    }

    ///Use 'latest' to automatically select the most recent snapshot
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Use 'latest' to automatically select the most recent
    /// snapshot",
    ///  "type": "string",
    ///  "enum": [
    ///    "latest"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchHistoryDiffToTimestampString {
        #[serde(rename = "latest")]
        Latest,
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffToTimestampString {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Latest => f.write_str("latest"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffToTimestampString {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "latest" => Ok(Self::Latest),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffToTimestampString {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffToTimestampString {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffToTimestampString {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`GetWatchHistoryDiffType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "diffLines",
    ///  "type": "string",
    ///  "enum": [
    ///    "diffLines",
    ///    "diffWords"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchHistoryDiffType {
        #[serde(rename = "diffLines")]
        DiffLines,
        #[serde(rename = "diffWords")]
        DiffWords,
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::DiffLines => f.write_str("diffLines"),
                Self::DiffWords => f.write_str("diffWords"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "diffLines" => Ok(Self::DiffLines),
                "diffWords" => Ok(Self::DiffWords),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for GetWatchHistoryDiffType {
        fn default() -> Self {
            GetWatchHistoryDiffType::DiffLines
        }
    }

    ///`GetWatchHistoryDiffWordDiff`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "false",
    ///  "type": "string",
    ///  "enum": [
    ///    "true",
    ///    "false",
    ///    "1",
    ///    "0",
    ///    "yes",
    ///    "no",
    ///    "on",
    ///    "off"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchHistoryDiffWordDiff {
        #[serde(rename = "true")]
        True,
        #[serde(rename = "false")]
        False,
        #[serde(rename = "1")]
        X1,
        #[serde(rename = "0")]
        X0,
        #[serde(rename = "yes")]
        Yes,
        #[serde(rename = "no")]
        No,
        #[serde(rename = "on")]
        On,
        #[serde(rename = "off")]
        Off,
    }

    impl ::std::fmt::Display for GetWatchHistoryDiffWordDiff {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::True => f.write_str("true"),
                Self::False => f.write_str("false"),
                Self::X1 => f.write_str("1"),
                Self::X0 => f.write_str("0"),
                Self::Yes => f.write_str("yes"),
                Self::No => f.write_str("no"),
                Self::On => f.write_str("on"),
                Self::Off => f.write_str("off"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchHistoryDiffWordDiff {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "true" => Ok(Self::True),
                "false" => Ok(Self::False),
                "1" => Ok(Self::X1),
                "0" => Ok(Self::X0),
                "yes" => Ok(Self::Yes),
                "no" => Ok(Self::No),
                "on" => Ok(Self::On),
                "off" => Ok(Self::Off),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchHistoryDiffWordDiff {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchHistoryDiffWordDiff {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchHistoryDiffWordDiff {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for GetWatchHistoryDiffWordDiff {
        fn default() -> Self {
            GetWatchHistoryDiffWordDiff::False
        }
    }

    ///`GetWatchMuted`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "muted",
    ///    "unmuted"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchMuted {
        #[serde(rename = "muted")]
        Muted,
        #[serde(rename = "unmuted")]
        Unmuted,
    }

    impl ::std::fmt::Display for GetWatchMuted {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Muted => f.write_str("muted"),
                Self::Unmuted => f.write_str("unmuted"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchMuted {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "muted" => Ok(Self::Muted),
                "unmuted" => Ok(Self::Unmuted),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchMuted {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchMuted {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchMuted {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`GetWatchPaused`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "paused",
    ///    "unpaused"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchPaused {
        #[serde(rename = "paused")]
        Paused,
        #[serde(rename = "unpaused")]
        Unpaused,
    }

    impl ::std::fmt::Display for GetWatchPaused {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Paused => f.write_str("paused"),
                Self::Unpaused => f.write_str("unpaused"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchPaused {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "paused" => Ok(Self::Paused),
                "unpaused" => Ok(Self::Unpaused),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchPaused {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchPaused {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchPaused {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`GetWatchRecheck`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "1",
    ///    "true"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchRecheck {
        #[serde(rename = "1")]
        X1,
        #[serde(rename = "true")]
        True,
    }

    impl ::std::fmt::Display for GetWatchRecheck {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X1 => f.write_str("1"),
                Self::True => f.write_str("true"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchRecheck {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "1" => Ok(Self::X1),
                "true" => Ok(Self::True),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchRecheck {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchRecheck {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchRecheck {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`GetWatchSnapshotHtml`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "1"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchSnapshotHtml {
        #[serde(rename = "1")]
        X1,
    }

    impl ::std::fmt::Display for GetWatchSnapshotHtml {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X1 => f.write_str("1"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchSnapshotHtml {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "1" => Ok(Self::X1),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchSnapshotHtml {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchSnapshotHtml {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchSnapshotHtml {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`GetWatchSnapshotTimestamp`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "type": "integer"
    ///    },
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "latest"
    ///      ]
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum GetWatchSnapshotTimestamp {
        Integer(i64),
        String(GetWatchSnapshotTimestampString),
    }

    impl ::std::str::FromStr for GetWatchSnapshotTimestamp {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if let Ok(v) = value.parse() {
                Ok(Self::Integer(v))
            } else if let Ok(v) = value.parse() {
                Ok(Self::String(v))
            } else {
                Err("string conversion failed for all variants".into())
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchSnapshotTimestamp {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchSnapshotTimestamp {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchSnapshotTimestamp {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for GetWatchSnapshotTimestamp {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                Self::Integer(x) => x.fmt(f),
                Self::String(x) => x.fmt(f),
            }
        }
    }

    impl ::std::convert::From<i64> for GetWatchSnapshotTimestamp {
        fn from(value: i64) -> Self {
            Self::Integer(value)
        }
    }

    impl ::std::convert::From<GetWatchSnapshotTimestampString> for GetWatchSnapshotTimestamp {
        fn from(value: GetWatchSnapshotTimestampString) -> Self {
            Self::String(value)
        }
    }

    ///`GetWatchSnapshotTimestampString`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "latest"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum GetWatchSnapshotTimestampString {
        #[serde(rename = "latest")]
        Latest,
    }

    impl ::std::fmt::Display for GetWatchSnapshotTimestampString {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Latest => f.write_str("latest"),
            }
        }
    }

    impl ::std::str::FromStr for GetWatchSnapshotTimestampString {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "latest" => Ok(Self::Latest),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetWatchSnapshotTimestampString {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetWatchSnapshotTimestampString {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetWatchSnapshotTimestampString {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`ListWatchesRecheckAll`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "1"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum ListWatchesRecheckAll {
        #[serde(rename = "1")]
        X1,
    }

    impl ::std::fmt::Display for ListWatchesRecheckAll {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X1 => f.write_str("1"),
            }
        }
    }

    impl ::std::str::FromStr for ListWatchesRecheckAll {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "1" => Ok(Self::X1),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for ListWatchesRecheckAll {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ListWatchesRecheckAll {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ListWatchesRecheckAll {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`NotificationUrls`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "notification_urls"
    ///  ],
    ///  "properties": {
    ///    "notification_urls": {
    ///      "description": "List of notification URLs",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "format": "uri"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct NotificationUrls {
        ///List of notification URLs
        pub notification_urls: ::std::vec::Vec<::std::string::String>,
    }

    ///`SearchResult`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "watches": {
    ///      "description": "Dictionary of matching web page change monitors
    /// (watches) keyed by UUID",
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "$ref": "#/components/schemas/Watch"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SearchResult {
        ///Dictionary of matching web page change monitors (watches) keyed by
        /// UUID
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub watches: ::std::collections::HashMap<::std::string::String, Watch>,
    }

    impl ::std::default::Default for SearchResult {
        fn default() -> Self {
            Self {
                watches: Default::default(),
            }
        }
    }

    ///`SystemInfo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "tag_count": {
    ///      "description": "Total number of tags",
    ///      "type": "integer"
    ///    },
    ///    "uptime": {
    ///      "description": "System uptime",
    ///      "type": "string"
    ///    },
    ///    "version": {
    ///      "description": "Application version",
    ///      "type": "string"
    ///    },
    ///    "watch_count": {
    ///      "description": "Total number of web page change monitors
    /// (watches)",
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SystemInfo {
        ///Total number of tags
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tag_count: ::std::option::Option<i64>,
        ///System uptime
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub uptime: ::std::option::Option<::std::string::String>,
        ///Application version
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub version: ::std::option::Option<::std::string::String>,
        ///Total number of web page change monitors (watches)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub watch_count: ::std::option::Option<i64>,
    }

    impl ::std::default::Default for SystemInfo {
        fn default() -> Self {
            Self {
                tag_count: Default::default(),
                uptime: Default::default(),
                version: Default::default(),
                watch_count: Default::default(),
            }
        }
    }

    ///`Tag`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/WatchBase"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "overrides_watch": {
    ///          "description": "Whether this tag's settings override watch
    /// settings for all watches in this tag/group.\n- true: Tag settings
    /// override watch settings\n- false: Tag settings do not override (watches
    /// use their own settings)\n- null: Not decided yet / inherit default
    /// behavior\n",
    ///          "type": [
    ///            "boolean",
    ///            "null"
    ///          ]
    ///        },
    ///        "url_match_pattern": {
    ///          "description": "Automatically apply this tag to any watch whose URL matches this pattern.\nSupports fnmatch wildcards (* and ?): e.g. *://example.com/* or github.com/myorg.\nPlain strings are matched as case-insensitive substrings.\nLeave empty to disable auto-matching.\n",
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct Tag {
        ///HTTP request body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub body: ::std::option::Option<TagBody>,
        ///Browser automation steps. Maximum 100 steps allowed.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub browser_steps: ::std::vec::Vec<TagBrowserStepsItem>,
        ///Compare against all history for unique lines
        #[serde(default)]
        pub check_unique_lines: bool,
        ///Array of condition rules for change detection logic (empty array
        /// when not set)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub conditions: ::std::vec::Vec<TagConditionsItem>,
        ///Logic operator - ALL (match all conditions) or ANY (match any
        /// condition)
        #[serde(default = "defaults::tag_conditions_match_logic")]
        pub conditions_match_logic: TagConditionsMatchLogic,
        ///Unix timestamp of creation
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub date_created: ::std::option::Option<i64>,
        ///Keep only lines containing these substrings (plain text,
        /// case-insensitive) — simpler alternative to regex
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_lines_containing: ::std::vec::Vec<TagExtractLinesContainingItem>,
        ///Regex patterns to extract specific text after filtering
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_text: ::std::vec::Vec<TagExtractTextItem>,
        ///Backend to use for fetching content. Common values:
        /// - `system` (default) - Use the system-wide default fetcher
        /// - `html_requests` - Fast requests-based fetcher
        /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
        /// - `extra_browser_*` - Custom browser configurations (if configured)
        /// - Plugin-provided fetchers (if installed)
        #[serde(default = "defaults::tag_fetch_backend")]
        pub fetch_backend: TagFetchBackend,
        ///Send notification when filters fail to match content
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_failure_notification_send: bool,
        ///Include added text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_added: bool,
        ///Include removed text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_removed: bool,
        ///Include replaced text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_replaced: bool,
        ///Monitor and track price changes (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub follow_price_changes: bool,
        ///Whether page has LD-JSON price data (auto-detected)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub has_ldjson_price_data: ::std::option::Option<bool>,
        ///HTTP headers to include in requests
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub headers: ::std::collections::HashMap<::std::string::String, ::std::string::String>,
        ///Maximum number of history snapshots to keep (null = use system
        /// default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub history_snapshot_max_length: ::std::option::Option<::std::num::NonZeroU64>,
        ///Ignore HTTP status code errors (boolean or null)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ignore_status_codes: ::std::option::Option<bool>,
        ///Text patterns to ignore in change detection
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub ignore_text: ::std::vec::Vec<TagIgnoreTextItem>,
        ///Only trigger on in-stock transitions (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub in_stock_only: bool,
        ///CSS/XPath selectors to extract specific content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub include_filters: ::std::vec::Vec<TagIncludeFiltersItem>,
        ///Internal cache of AI evaluation results keyed by (intent, diff) hash
        /// (auto-managed).
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub llm_evaluation_cache: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        ///Number of tokens consumed by the AI on the most recent check.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_last_tokens_used: ::std::option::Option<i64>,
        ///CSS selector derived by the AI to narrow content scope before
        /// evaluation (auto-managed, do not set manually).
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_prefilter: ::std::option::Option<::std::string::String>,
        ///Total tokens consumed by the AI across all checks for this watch.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_tokens_used_cumulative: ::std::option::Option<i64>,
        ///HTTP method to use
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub method: ::std::option::Option<TagMethod>,
        ///Custom notification body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_body: ::std::option::Option<TagNotificationBody>,
        ///Format for notifications
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_format: ::std::option::Option<TagNotificationFormat>,
        ///Whether notifications are muted
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_muted: ::std::option::Option<bool>,
        ///Include screenshot in notifications (if supported by notification
        /// URL)
        #[serde(default)]
        pub notification_screenshot: bool,
        ///Custom notification title
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_title: ::std::option::Option<TagNotificationTitle>,
        ///Notification URLs for this web page change monitor (watch). Maximum
        /// 100 URLs.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub notification_urls: ::std::vec::Vec<TagNotificationUrlsItem>,
        ///Whether this tag's settings override watch settings for all watches
        /// in this tag/group.
        /// - true: Tag settings override watch settings
        /// - false: Tag settings do not override (watches use their own
        ///   settings)
        /// - null: Not decided yet / inherit default behavior
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub overrides_watch: ::std::option::Option<bool>,
        ///Whether the web page change monitor (watch) is paused
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub paused: ::std::option::Option<bool>,
        ///Minimum price change percentage to trigger notification
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_change_threshold_percent: ::std::option::Option<f64>,
        ///Optional processor mode to use for change detection. Defaults to
        /// `text_json_diff` if not specified.
        #[serde(default = "defaults::tag_processor")]
        pub processor: TagProcessor,
        ///Proxy configuration
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxy: ::std::option::Option<TagProxy>,
        ///Remove duplicate lines from content
        #[serde(default)]
        pub remove_duplicate_lines: bool,
        ///Sort lines alphabetically before comparison
        #[serde(default)]
        pub sort_text_alphabetically: bool,
        ///Remove lines matching ignore patterns
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub strip_ignored_lines: ::std::option::Option<bool>,
        ///CSS/XPath selectors to remove content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub subtractive_selectors: ::std::vec::Vec<TagSubtractiveSelectorsItem>,
        ///Tag UUID to associate with this web page change monitor (watch)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tag: ::std::option::Option<TagTag>,
        ///Array of tag UUIDs
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///Text that should NOT be present (triggers alert if found)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub text_should_not_be_present: ::std::vec::Vec<TagTextShouldNotBePresentItem>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_between_check: ::std::option::Option<TagTimeBetweenCheck>,
        ///Whether to use global settings for time between checks - defaults to
        /// true if not set
        #[serde(default = "defaults::default_bool::<true>")]
        pub time_between_check_use_default: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_schedule_limit: ::std::option::Option<TagTimeScheduleLimit>,
        ///Custom title for the web page change monitor (watch), not to be
        /// confused with page_title
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub title: ::std::option::Option<TagTitle>,
        ///Whether to track JSON-LD price data
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub track_ldjson_price_data: ::std::option::Option<bool>,
        ///Text/regex patterns that must be present to trigger a change
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub trigger_text: ::std::vec::Vec<TagTriggerTextItem>,
        ///Strip leading/trailing whitespace from text
        #[serde(default)]
        pub trim_text_whitespace: bool,
        ///URL to monitor for changes
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub url: ::std::option::Option<::std::string::String>,
        ///Automatically apply this tag to any watch whose URL matches this
        /// pattern. Supports fnmatch wildcards (* and ?): e.g.
        /// *://example.com/* or github.com/myorg. Plain strings are
        /// matched as case-insensitive substrings. Leave empty to
        /// disable auto-matching.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub url_match_pattern: ::std::option::Option<::std::string::String>,
        ///Display page title in watch list (null = use system default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub use_page_title_in_list: ::std::option::Option<bool>,
        ///Unique identifier
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub uuid: ::std::option::Option<::uuid::Uuid>,
        ///Delay in seconds for webdriver
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_delay: ::std::option::Option<i64>,
        ///JavaScript code to execute
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_js_execute_code: ::std::option::Option<TagWebdriverJsExecuteCode>,
    }

    impl ::std::default::Default for Tag {
        fn default() -> Self {
            Self {
                body: Default::default(),
                browser_steps: Default::default(),
                check_unique_lines: Default::default(),
                conditions: Default::default(),
                conditions_match_logic: defaults::tag_conditions_match_logic(),
                date_created: Default::default(),
                extract_lines_containing: Default::default(),
                extract_text: Default::default(),
                fetch_backend: defaults::tag_fetch_backend(),
                filter_failure_notification_send: defaults::default_bool::<true>(),
                filter_text_added: defaults::default_bool::<true>(),
                filter_text_removed: defaults::default_bool::<true>(),
                filter_text_replaced: defaults::default_bool::<true>(),
                follow_price_changes: defaults::default_bool::<true>(),
                has_ldjson_price_data: Default::default(),
                headers: Default::default(),
                history_snapshot_max_length: Default::default(),
                ignore_status_codes: Default::default(),
                ignore_text: Default::default(),
                in_stock_only: defaults::default_bool::<true>(),
                include_filters: Default::default(),
                llm_evaluation_cache: Default::default(),
                llm_last_tokens_used: Default::default(),
                llm_prefilter: Default::default(),
                llm_tokens_used_cumulative: Default::default(),
                method: Default::default(),
                notification_body: Default::default(),
                notification_format: Default::default(),
                notification_muted: Default::default(),
                notification_screenshot: Default::default(),
                notification_title: Default::default(),
                notification_urls: Default::default(),
                overrides_watch: Default::default(),
                paused: Default::default(),
                price_change_threshold_percent: Default::default(),
                processor: defaults::tag_processor(),
                proxy: Default::default(),
                remove_duplicate_lines: Default::default(),
                sort_text_alphabetically: Default::default(),
                strip_ignored_lines: Default::default(),
                subtractive_selectors: Default::default(),
                tag: Default::default(),
                tags: Default::default(),
                text_should_not_be_present: Default::default(),
                time_between_check: Default::default(),
                time_between_check_use_default: defaults::default_bool::<true>(),
                time_schedule_limit: Default::default(),
                title: Default::default(),
                track_ldjson_price_data: Default::default(),
                trigger_text: Default::default(),
                trim_text_whitespace: Default::default(),
                url: Default::default(),
                url_match_pattern: Default::default(),
                use_page_title_in_list: Default::default(),
                uuid: Default::default(),
                webdriver_delay: Default::default(),
                webdriver_js_execute_code: Default::default(),
            }
        }
    }

    ///HTTP request body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP request body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagBody(::std::string::String);
    impl ::std::ops::Deref for TagBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagBody> for ::std::string::String {
        fn from(value: TagBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`TagBrowserStepsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "operation",
    ///    "optional_value",
    ///    "selector"
    ///  ],
    ///  "properties": {
    ///    "operation": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "optional_value": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "selector": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct TagBrowserStepsItem {
        pub operation: ::std::option::Option<TagBrowserStepsItemOperation>,
        pub optional_value: ::std::option::Option<TagBrowserStepsItemOptionalValue>,
        pub selector: ::std::option::Option<TagBrowserStepsItemSelector>,
    }

    ///`TagBrowserStepsItemOperation`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagBrowserStepsItemOperation(::std::string::String);
    impl ::std::ops::Deref for TagBrowserStepsItemOperation {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagBrowserStepsItemOperation> for ::std::string::String {
        fn from(value: TagBrowserStepsItemOperation) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagBrowserStepsItemOperation {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagBrowserStepsItemOperation {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`TagBrowserStepsItemOptionalValue`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagBrowserStepsItemOptionalValue(::std::string::String);
    impl ::std::ops::Deref for TagBrowserStepsItemOptionalValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagBrowserStepsItemOptionalValue> for ::std::string::String {
        fn from(value: TagBrowserStepsItemOptionalValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagBrowserStepsItemOptionalValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagBrowserStepsItemOptionalValue {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`TagBrowserStepsItemSelector`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagBrowserStepsItemSelector(::std::string::String);
    impl ::std::ops::Deref for TagBrowserStepsItemSelector {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagBrowserStepsItemSelector> for ::std::string::String {
        fn from(value: TagBrowserStepsItemSelector) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagBrowserStepsItemSelector {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagBrowserStepsItemSelector {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`TagConditionsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "field",
    ///    "operator",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "field": {
    ///      "description": "Field to check (e.g., 'page_filtered_text',
    /// 'page_title')",
    ///      "type": "string"
    ///    },
    ///    "operator": {
    ///      "description": "Comparison operator (e.g., 'contains_regex',
    /// 'equals', 'not_equals')",
    ///      "type": "string"
    ///    },
    ///    "value": {
    ///      "description": "Value to compare against",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TagConditionsItem {
        ///Field to check (e.g., 'page_filtered_text', 'page_title')
        pub field: ::std::string::String,
        ///Comparison operator (e.g., 'contains_regex', 'equals', 'not_equals')
        pub operator: ::std::string::String,
        ///Value to compare against
        pub value: ::std::string::String,
    }

    ///Logic operator - ALL (match all conditions) or ANY (match any condition)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Logic operator - ALL (match all conditions) or ANY
    /// (match any condition)",
    ///  "default": "ALL",
    ///  "type": "string",
    ///  "enum": [
    ///    "ALL",
    ///    "ANY"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum TagConditionsMatchLogic {
        #[serde(rename = "ALL")]
        All,
        #[serde(rename = "ANY")]
        Any,
    }

    impl ::std::fmt::Display for TagConditionsMatchLogic {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::All => f.write_str("ALL"),
                Self::Any => f.write_str("ANY"),
            }
        }
    }

    impl ::std::str::FromStr for TagConditionsMatchLogic {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "ALL" => Ok(Self::All),
                "ANY" => Ok(Self::Any),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TagConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for TagConditionsMatchLogic {
        fn default() -> Self {
            TagConditionsMatchLogic::All
        }
    }

    ///`TagExtractLinesContainingItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagExtractLinesContainingItem(::std::string::String);
    impl ::std::ops::Deref for TagExtractLinesContainingItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagExtractLinesContainingItem> for ::std::string::String {
        fn from(value: TagExtractLinesContainingItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagExtractLinesContainingItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagExtractLinesContainingItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`TagExtractTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagExtractTextItem(::std::string::String);
    impl ::std::ops::Deref for TagExtractTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagExtractTextItem> for ::std::string::String {
        fn from(value: TagExtractTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagExtractTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagExtractTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Backend to use for fetching content. Common values:
    /// - `system` (default) - Use the system-wide default fetcher
    /// - `html_requests` - Fast requests-based fetcher
    /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
    /// - `extra_browser_*` - Custom browser configurations (if configured)
    /// - Plugin-provided fetchers (if installed)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Backend to use for fetching content. Common values:\n-
    /// `system` (default) - Use the system-wide default fetcher\n-
    /// `html_requests` - Fast requests-based fetcher\n- `html_webdriver` -
    /// Browser-based fetcher (Playwright/Puppeteer)\n- `extra_browser_*` -
    /// Custom browser configurations (if configured)\n- Plugin-provided
    /// fetchers (if installed)\n",
    ///  "default": "system",
    ///  "type": "string",
    ///  "pattern": "^(system|html_requests|html_webdriver|extra_browser_.+)$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagFetchBackend(::std::string::String);
    impl ::std::ops::Deref for TagFetchBackend {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagFetchBackend> for ::std::string::String {
        fn from(value: TagFetchBackend) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for TagFetchBackend {
        fn default() -> Self {
            TagFetchBackend("system".to_string())
        }
    }

    impl ::std::str::FromStr for TagFetchBackend {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new(
                        "^(system|html_requests|html_webdriver|extra_browser_.+)$",
                    )
                    .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^(system|html_requests|html_webdriver|extra_browser_.+)$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagFetchBackend {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`TagIgnoreTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagIgnoreTextItem(::std::string::String);
    impl ::std::ops::Deref for TagIgnoreTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagIgnoreTextItem> for ::std::string::String {
        fn from(value: TagIgnoreTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagIgnoreTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagIgnoreTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`TagIncludeFiltersItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagIncludeFiltersItem(::std::string::String);
    impl ::std::ops::Deref for TagIncludeFiltersItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagIncludeFiltersItem> for ::std::string::String {
        fn from(value: TagIncludeFiltersItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagIncludeFiltersItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagIncludeFiltersItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///HTTP method to use
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP method to use",
    ///  "type": "string",
    ///  "enum": [
    ///    "GET",
    ///    "POST",
    ///    "DELETE",
    ///    "PUT"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum TagMethod {
        #[serde(rename = "GET")]
        Get,
        #[serde(rename = "POST")]
        Post,
        #[serde(rename = "DELETE")]
        Delete,
        #[serde(rename = "PUT")]
        Put,
    }

    impl ::std::fmt::Display for TagMethod {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Get => f.write_str("GET"),
                Self::Post => f.write_str("POST"),
                Self::Delete => f.write_str("DELETE"),
                Self::Put => f.write_str("PUT"),
            }
        }
    }

    impl ::std::str::FromStr for TagMethod {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "GET" => Ok(Self::Get),
                "POST" => Ok(Self::Post),
                "DELETE" => Ok(Self::Delete),
                "PUT" => Ok(Self::Put),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TagMethod {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagNotificationBody(::std::string::String);
    impl ::std::ops::Deref for TagNotificationBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagNotificationBody> for ::std::string::String {
        fn from(value: TagNotificationBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagNotificationBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagNotificationBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Format for notifications
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Format for notifications",
    ///  "type": "string",
    ///  "enum": [
    ///    "text",
    ///    "html",
    ///    "htmlcolor",
    ///    "markdown",
    ///    "System default"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum TagNotificationFormat {
        #[serde(rename = "text")]
        Text,
        #[serde(rename = "html")]
        Html,
        #[serde(rename = "htmlcolor")]
        Htmlcolor,
        #[serde(rename = "markdown")]
        Markdown,
        #[serde(rename = "System default")]
        SystemDefault,
    }

    impl ::std::fmt::Display for TagNotificationFormat {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Text => f.write_str("text"),
                Self::Html => f.write_str("html"),
                Self::Htmlcolor => f.write_str("htmlcolor"),
                Self::Markdown => f.write_str("markdown"),
                Self::SystemDefault => f.write_str("System default"),
            }
        }
    }

    impl ::std::str::FromStr for TagNotificationFormat {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "text" => Ok(Self::Text),
                "html" => Ok(Self::Html),
                "htmlcolor" => Ok(Self::Htmlcolor),
                "markdown" => Ok(Self::Markdown),
                "System default" => Ok(Self::SystemDefault),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TagNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagNotificationTitle(::std::string::String);
    impl ::std::ops::Deref for TagNotificationTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagNotificationTitle> for ::std::string::String {
        fn from(value: TagNotificationTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagNotificationTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagNotificationTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`TagNotificationUrlsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 1000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagNotificationUrlsItem(::std::string::String);
    impl ::std::ops::Deref for TagNotificationUrlsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagNotificationUrlsItem> for ::std::string::String {
        fn from(value: TagNotificationUrlsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagNotificationUrlsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 1000usize {
                return Err("longer than 1000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagNotificationUrlsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Optional processor mode to use for change detection. Defaults to
    /// `text_json_diff` if not specified.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Optional processor mode to use for change detection.
    /// Defaults to `text_json_diff` if not specified.",
    ///  "default": "text_json_diff",
    ///  "type": "string",
    ///  "enum": [
    ///    "restock_diff",
    ///    "text_json_diff"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum TagProcessor {
        #[serde(rename = "restock_diff")]
        RestockDiff,
        #[serde(rename = "text_json_diff")]
        TextJsonDiff,
    }

    impl ::std::fmt::Display for TagProcessor {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::RestockDiff => f.write_str("restock_diff"),
                Self::TextJsonDiff => f.write_str("text_json_diff"),
            }
        }
    }

    impl ::std::str::FromStr for TagProcessor {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "restock_diff" => Ok(Self::RestockDiff),
                "text_json_diff" => Ok(Self::TextJsonDiff),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TagProcessor {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for TagProcessor {
        fn default() -> Self {
            TagProcessor::TextJsonDiff
        }
    }

    ///Proxy configuration
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Proxy configuration",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagProxy(::std::string::String);
    impl ::std::ops::Deref for TagProxy {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagProxy> for ::std::string::String {
        fn from(value: TagProxy) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagProxy {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagProxy {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagProxy {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`TagSubtractiveSelectorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagSubtractiveSelectorsItem(::std::string::String);
    impl ::std::ops::Deref for TagSubtractiveSelectorsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagSubtractiveSelectorsItem> for ::std::string::String {
        fn from(value: TagSubtractiveSelectorsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagSubtractiveSelectorsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagSubtractiveSelectorsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Tag UUID to associate with this web page change monitor (watch)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Tag UUID to associate with this web page change monitor
    /// (watch)",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagTag(::std::string::String);
    impl ::std::ops::Deref for TagTag {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagTag> for ::std::string::String {
        fn from(value: TagTag) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagTag {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagTag {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagTag {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`TagTextShouldNotBePresentItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagTextShouldNotBePresentItem(::std::string::String);
    impl ::std::ops::Deref for TagTextShouldNotBePresentItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagTextShouldNotBePresentItem> for ::std::string::String {
        fn from(value: TagTextShouldNotBePresentItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagTextShouldNotBePresentItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagTextShouldNotBePresentItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Time intervals between checks. All fields must be non-negative. At least
    /// one non-zero value required when not using default settings.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Time intervals between checks. All fields must be
    /// non-negative. At least one non-zero value required when not using
    /// default settings.",
    ///  "type": "object",
    ///  "properties": {
    ///    "days": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 365000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "hours": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 8760000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "minutes": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 525600000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "seconds": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 31536000000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "weeks": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 52000.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TagTimeBetweenCheck {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub days: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub hours: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub minutes: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub seconds: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub weeks: ::std::option::Option<i64>,
    }

    impl ::std::default::Default for TagTimeBetweenCheck {
        fn default() -> Self {
            Self {
                days: Default::default(),
                hours: Default::default(),
                minutes: Default::default(),
                seconds: Default::default(),
                weeks: Default::default(),
            }
        }
    }

    ///Weekly schedule limiting when checks can run
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Weekly schedule limiting when checks can run",
    ///  "type": "object",
    ///  "properties": {
    ///    "enabled": {
    ///      "default": false,
    ///      "type": "boolean"
    ///    },
    ///    "friday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "monday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "saturday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "sunday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "thursday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "tuesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "wednesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TagTimeScheduleLimit {
        #[serde(default)]
        pub enabled: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub friday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub monday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub saturday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub sunday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub thursday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tuesday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub wednesday: ::std::option::Option<DaySchedule>,
    }

    impl ::std::default::Default for TagTimeScheduleLimit {
        fn default() -> Self {
            Self {
                enabled: Default::default(),
                friday: Default::default(),
                monday: Default::default(),
                saturday: Default::default(),
                sunday: Default::default(),
                thursday: Default::default(),
                tuesday: Default::default(),
                wednesday: Default::default(),
            }
        }
    }

    ///Custom title for the web page change monitor (watch), not to be confused
    /// with page_title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom title for the web page change monitor (watch),
    /// not to be confused with page_title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagTitle(::std::string::String);
    impl ::std::ops::Deref for TagTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagTitle> for ::std::string::String {
        fn from(value: TagTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`TagTriggerTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagTriggerTextItem(::std::string::String);
    impl ::std::ops::Deref for TagTriggerTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagTriggerTextItem> for ::std::string::String {
        fn from(value: TagTriggerTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagTriggerTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagTriggerTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///JavaScript code to execute
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "JavaScript code to execute",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TagWebdriverJsExecuteCode(::std::string::String);
    impl ::std::ops::Deref for TagWebdriverJsExecuteCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TagWebdriverJsExecuteCode> for ::std::string::String {
        fn from(value: TagWebdriverJsExecuteCode) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TagWebdriverJsExecuteCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TagWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TagWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TagWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TagWebdriverJsExecuteCode {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatch`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/WatchBase"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "last_viewed": {
    ///          "description": "Unix timestamp in seconds of the last time the
    /// watch was viewed. Setting it to a value higher than `last_changed` in
    /// the \"Update watch\" endpoint marks the watch as viewed.",
    ///          "type": "integer",
    ///          "minimum": 0.0
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct UpdateWatch {
        ///HTTP request body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub body: ::std::option::Option<UpdateWatchBody>,
        ///Browser automation steps. Maximum 100 steps allowed.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub browser_steps: ::std::vec::Vec<UpdateWatchBrowserStepsItem>,
        ///Compare against all history for unique lines
        #[serde(default)]
        pub check_unique_lines: bool,
        ///Array of condition rules for change detection logic (empty array
        /// when not set)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub conditions: ::std::vec::Vec<UpdateWatchConditionsItem>,
        ///Logic operator - ALL (match all conditions) or ANY (match any
        /// condition)
        #[serde(default = "defaults::update_watch_conditions_match_logic")]
        pub conditions_match_logic: UpdateWatchConditionsMatchLogic,
        ///Unix timestamp of creation
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub date_created: ::std::option::Option<i64>,
        ///Keep only lines containing these substrings (plain text,
        /// case-insensitive) — simpler alternative to regex
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_lines_containing: ::std::vec::Vec<UpdateWatchExtractLinesContainingItem>,
        ///Regex patterns to extract specific text after filtering
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_text: ::std::vec::Vec<UpdateWatchExtractTextItem>,
        ///Backend to use for fetching content. Common values:
        /// - `system` (default) - Use the system-wide default fetcher
        /// - `html_requests` - Fast requests-based fetcher
        /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
        /// - `extra_browser_*` - Custom browser configurations (if configured)
        /// - Plugin-provided fetchers (if installed)
        #[serde(default = "defaults::update_watch_fetch_backend")]
        pub fetch_backend: UpdateWatchFetchBackend,
        ///Send notification when filters fail to match content
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_failure_notification_send: bool,
        ///Include added text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_added: bool,
        ///Include removed text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_removed: bool,
        ///Include replaced text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_replaced: bool,
        ///Monitor and track price changes (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub follow_price_changes: bool,
        ///Whether page has LD-JSON price data (auto-detected)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub has_ldjson_price_data: ::std::option::Option<bool>,
        ///HTTP headers to include in requests
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub headers: ::std::collections::HashMap<::std::string::String, ::std::string::String>,
        ///Maximum number of history snapshots to keep (null = use system
        /// default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub history_snapshot_max_length: ::std::option::Option<::std::num::NonZeroU64>,
        ///Ignore HTTP status code errors (boolean or null)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ignore_status_codes: ::std::option::Option<bool>,
        ///Text patterns to ignore in change detection
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub ignore_text: ::std::vec::Vec<UpdateWatchIgnoreTextItem>,
        ///Only trigger on in-stock transitions (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub in_stock_only: bool,
        ///CSS/XPath selectors to extract specific content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub include_filters: ::std::vec::Vec<UpdateWatchIncludeFiltersItem>,
        ///Unix timestamp in seconds of the last time the watch was viewed.
        /// Setting it to a value higher than `last_changed` in the "Update
        /// watch" endpoint marks the watch as viewed.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub last_viewed: ::std::option::Option<u64>,
        ///Internal cache of AI evaluation results keyed by (intent, diff) hash
        /// (auto-managed).
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub llm_evaluation_cache: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        ///Number of tokens consumed by the AI on the most recent check.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_last_tokens_used: ::std::option::Option<i64>,
        ///CSS selector derived by the AI to narrow content scope before
        /// evaluation (auto-managed, do not set manually).
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_prefilter: ::std::option::Option<::std::string::String>,
        ///Total tokens consumed by the AI across all checks for this watch.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_tokens_used_cumulative: ::std::option::Option<i64>,
        ///HTTP method to use
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub method: ::std::option::Option<UpdateWatchMethod>,
        ///Custom notification body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_body: ::std::option::Option<UpdateWatchNotificationBody>,
        ///Format for notifications
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_format: ::std::option::Option<UpdateWatchNotificationFormat>,
        ///Whether notifications are muted
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_muted: ::std::option::Option<bool>,
        ///Include screenshot in notifications (if supported by notification
        /// URL)
        #[serde(default)]
        pub notification_screenshot: bool,
        ///Custom notification title
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_title: ::std::option::Option<UpdateWatchNotificationTitle>,
        ///Notification URLs for this web page change monitor (watch). Maximum
        /// 100 URLs.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub notification_urls: ::std::vec::Vec<UpdateWatchNotificationUrlsItem>,
        ///Whether the web page change monitor (watch) is paused
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub paused: ::std::option::Option<bool>,
        ///Minimum price change percentage to trigger notification
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_change_threshold_percent: ::std::option::Option<f64>,
        ///Optional processor mode to use for change detection. Defaults to
        /// `text_json_diff` if not specified.
        #[serde(default = "defaults::update_watch_processor")]
        pub processor: UpdateWatchProcessor,
        ///Proxy configuration
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxy: ::std::option::Option<UpdateWatchProxy>,
        ///Remove duplicate lines from content
        #[serde(default)]
        pub remove_duplicate_lines: bool,
        ///Sort lines alphabetically before comparison
        #[serde(default)]
        pub sort_text_alphabetically: bool,
        ///Remove lines matching ignore patterns
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub strip_ignored_lines: ::std::option::Option<bool>,
        ///CSS/XPath selectors to remove content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub subtractive_selectors: ::std::vec::Vec<UpdateWatchSubtractiveSelectorsItem>,
        ///Tag UUID to associate with this web page change monitor (watch)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tag: ::std::option::Option<UpdateWatchTag>,
        ///Array of tag UUIDs
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///Text that should NOT be present (triggers alert if found)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub text_should_not_be_present: ::std::vec::Vec<UpdateWatchTextShouldNotBePresentItem>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_between_check: ::std::option::Option<UpdateWatchTimeBetweenCheck>,
        ///Whether to use global settings for time between checks - defaults to
        /// true if not set
        #[serde(default = "defaults::default_bool::<true>")]
        pub time_between_check_use_default: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_schedule_limit: ::std::option::Option<UpdateWatchTimeScheduleLimit>,
        ///Custom title for the web page change monitor (watch), not to be
        /// confused with page_title
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub title: ::std::option::Option<UpdateWatchTitle>,
        ///Whether to track JSON-LD price data
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub track_ldjson_price_data: ::std::option::Option<bool>,
        ///Text/regex patterns that must be present to trigger a change
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub trigger_text: ::std::vec::Vec<UpdateWatchTriggerTextItem>,
        ///Strip leading/trailing whitespace from text
        #[serde(default)]
        pub trim_text_whitespace: bool,
        ///URL to monitor for changes
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub url: ::std::option::Option<::std::string::String>,
        ///Display page title in watch list (null = use system default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub use_page_title_in_list: ::std::option::Option<bool>,
        ///Unique identifier
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub uuid: ::std::option::Option<::uuid::Uuid>,
        ///Delay in seconds for webdriver
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_delay: ::std::option::Option<i64>,
        ///JavaScript code to execute
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_js_execute_code: ::std::option::Option<UpdateWatchWebdriverJsExecuteCode>,
    }

    impl ::std::default::Default for UpdateWatch {
        fn default() -> Self {
            Self {
                body: Default::default(),
                browser_steps: Default::default(),
                check_unique_lines: Default::default(),
                conditions: Default::default(),
                conditions_match_logic: defaults::update_watch_conditions_match_logic(),
                date_created: Default::default(),
                extract_lines_containing: Default::default(),
                extract_text: Default::default(),
                fetch_backend: defaults::update_watch_fetch_backend(),
                filter_failure_notification_send: defaults::default_bool::<true>(),
                filter_text_added: defaults::default_bool::<true>(),
                filter_text_removed: defaults::default_bool::<true>(),
                filter_text_replaced: defaults::default_bool::<true>(),
                follow_price_changes: defaults::default_bool::<true>(),
                has_ldjson_price_data: Default::default(),
                headers: Default::default(),
                history_snapshot_max_length: Default::default(),
                ignore_status_codes: Default::default(),
                ignore_text: Default::default(),
                in_stock_only: defaults::default_bool::<true>(),
                include_filters: Default::default(),
                last_viewed: Default::default(),
                llm_evaluation_cache: Default::default(),
                llm_last_tokens_used: Default::default(),
                llm_prefilter: Default::default(),
                llm_tokens_used_cumulative: Default::default(),
                method: Default::default(),
                notification_body: Default::default(),
                notification_format: Default::default(),
                notification_muted: Default::default(),
                notification_screenshot: Default::default(),
                notification_title: Default::default(),
                notification_urls: Default::default(),
                paused: Default::default(),
                price_change_threshold_percent: Default::default(),
                processor: defaults::update_watch_processor(),
                proxy: Default::default(),
                remove_duplicate_lines: Default::default(),
                sort_text_alphabetically: Default::default(),
                strip_ignored_lines: Default::default(),
                subtractive_selectors: Default::default(),
                tag: Default::default(),
                tags: Default::default(),
                text_should_not_be_present: Default::default(),
                time_between_check: Default::default(),
                time_between_check_use_default: defaults::default_bool::<true>(),
                time_schedule_limit: Default::default(),
                title: Default::default(),
                track_ldjson_price_data: Default::default(),
                trigger_text: Default::default(),
                trim_text_whitespace: Default::default(),
                url: Default::default(),
                use_page_title_in_list: Default::default(),
                uuid: Default::default(),
                webdriver_delay: Default::default(),
                webdriver_js_execute_code: Default::default(),
            }
        }
    }

    ///HTTP request body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP request body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchBody(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchBody> for ::std::string::String {
        fn from(value: UpdateWatchBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatchBrowserStepsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "operation",
    ///    "optional_value",
    ///    "selector"
    ///  ],
    ///  "properties": {
    ///    "operation": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "optional_value": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "selector": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct UpdateWatchBrowserStepsItem {
        pub operation: ::std::option::Option<UpdateWatchBrowserStepsItemOperation>,
        pub optional_value: ::std::option::Option<UpdateWatchBrowserStepsItemOptionalValue>,
        pub selector: ::std::option::Option<UpdateWatchBrowserStepsItemSelector>,
    }

    ///`UpdateWatchBrowserStepsItemOperation`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchBrowserStepsItemOperation(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchBrowserStepsItemOperation {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchBrowserStepsItemOperation> for ::std::string::String {
        fn from(value: UpdateWatchBrowserStepsItemOperation) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchBrowserStepsItemOperation {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchBrowserStepsItemOperation {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatchBrowserStepsItemOptionalValue`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchBrowserStepsItemOptionalValue(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchBrowserStepsItemOptionalValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchBrowserStepsItemOptionalValue> for ::std::string::String {
        fn from(value: UpdateWatchBrowserStepsItemOptionalValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchBrowserStepsItemOptionalValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchBrowserStepsItemOptionalValue {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatchBrowserStepsItemSelector`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchBrowserStepsItemSelector(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchBrowserStepsItemSelector {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchBrowserStepsItemSelector> for ::std::string::String {
        fn from(value: UpdateWatchBrowserStepsItemSelector) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchBrowserStepsItemSelector {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchBrowserStepsItemSelector {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatchConditionsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "field",
    ///    "operator",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "field": {
    ///      "description": "Field to check (e.g., 'page_filtered_text',
    /// 'page_title')",
    ///      "type": "string"
    ///    },
    ///    "operator": {
    ///      "description": "Comparison operator (e.g., 'contains_regex',
    /// 'equals', 'not_equals')",
    ///      "type": "string"
    ///    },
    ///    "value": {
    ///      "description": "Value to compare against",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct UpdateWatchConditionsItem {
        ///Field to check (e.g., 'page_filtered_text', 'page_title')
        pub field: ::std::string::String,
        ///Comparison operator (e.g., 'contains_regex', 'equals', 'not_equals')
        pub operator: ::std::string::String,
        ///Value to compare against
        pub value: ::std::string::String,
    }

    ///Logic operator - ALL (match all conditions) or ANY (match any condition)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Logic operator - ALL (match all conditions) or ANY
    /// (match any condition)",
    ///  "default": "ALL",
    ///  "type": "string",
    ///  "enum": [
    ///    "ALL",
    ///    "ANY"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum UpdateWatchConditionsMatchLogic {
        #[serde(rename = "ALL")]
        All,
        #[serde(rename = "ANY")]
        Any,
    }

    impl ::std::fmt::Display for UpdateWatchConditionsMatchLogic {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::All => f.write_str("ALL"),
                Self::Any => f.write_str("ANY"),
            }
        }
    }

    impl ::std::str::FromStr for UpdateWatchConditionsMatchLogic {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "ALL" => Ok(Self::All),
                "ANY" => Ok(Self::Any),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for UpdateWatchConditionsMatchLogic {
        fn default() -> Self {
            UpdateWatchConditionsMatchLogic::All
        }
    }

    ///`UpdateWatchExtractLinesContainingItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchExtractLinesContainingItem(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchExtractLinesContainingItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchExtractLinesContainingItem> for ::std::string::String {
        fn from(value: UpdateWatchExtractLinesContainingItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchExtractLinesContainingItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchExtractLinesContainingItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatchExtractTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchExtractTextItem(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchExtractTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchExtractTextItem> for ::std::string::String {
        fn from(value: UpdateWatchExtractTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchExtractTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchExtractTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Backend to use for fetching content. Common values:
    /// - `system` (default) - Use the system-wide default fetcher
    /// - `html_requests` - Fast requests-based fetcher
    /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
    /// - `extra_browser_*` - Custom browser configurations (if configured)
    /// - Plugin-provided fetchers (if installed)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Backend to use for fetching content. Common values:\n-
    /// `system` (default) - Use the system-wide default fetcher\n-
    /// `html_requests` - Fast requests-based fetcher\n- `html_webdriver` -
    /// Browser-based fetcher (Playwright/Puppeteer)\n- `extra_browser_*` -
    /// Custom browser configurations (if configured)\n- Plugin-provided
    /// fetchers (if installed)\n",
    ///  "default": "system",
    ///  "type": "string",
    ///  "pattern": "^(system|html_requests|html_webdriver|extra_browser_.+)$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchFetchBackend(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchFetchBackend {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchFetchBackend> for ::std::string::String {
        fn from(value: UpdateWatchFetchBackend) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for UpdateWatchFetchBackend {
        fn default() -> Self {
            UpdateWatchFetchBackend("system".to_string())
        }
    }

    impl ::std::str::FromStr for UpdateWatchFetchBackend {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new(
                        "^(system|html_requests|html_webdriver|extra_browser_.+)$",
                    )
                    .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^(system|html_requests|html_webdriver|extra_browser_.+)$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchFetchBackend {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatchIgnoreTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchIgnoreTextItem(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchIgnoreTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchIgnoreTextItem> for ::std::string::String {
        fn from(value: UpdateWatchIgnoreTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchIgnoreTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchIgnoreTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatchIncludeFiltersItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchIncludeFiltersItem(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchIncludeFiltersItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchIncludeFiltersItem> for ::std::string::String {
        fn from(value: UpdateWatchIncludeFiltersItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchIncludeFiltersItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchIncludeFiltersItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///HTTP method to use
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP method to use",
    ///  "type": "string",
    ///  "enum": [
    ///    "GET",
    ///    "POST",
    ///    "DELETE",
    ///    "PUT"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum UpdateWatchMethod {
        #[serde(rename = "GET")]
        Get,
        #[serde(rename = "POST")]
        Post,
        #[serde(rename = "DELETE")]
        Delete,
        #[serde(rename = "PUT")]
        Put,
    }

    impl ::std::fmt::Display for UpdateWatchMethod {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Get => f.write_str("GET"),
                Self::Post => f.write_str("POST"),
                Self::Delete => f.write_str("DELETE"),
                Self::Put => f.write_str("PUT"),
            }
        }
    }

    impl ::std::str::FromStr for UpdateWatchMethod {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "GET" => Ok(Self::Get),
                "POST" => Ok(Self::Post),
                "DELETE" => Ok(Self::Delete),
                "PUT" => Ok(Self::Put),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchMethod {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchNotificationBody(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchNotificationBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchNotificationBody> for ::std::string::String {
        fn from(value: UpdateWatchNotificationBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchNotificationBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchNotificationBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Format for notifications
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Format for notifications",
    ///  "type": "string",
    ///  "enum": [
    ///    "text",
    ///    "html",
    ///    "htmlcolor",
    ///    "markdown",
    ///    "System default"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum UpdateWatchNotificationFormat {
        #[serde(rename = "text")]
        Text,
        #[serde(rename = "html")]
        Html,
        #[serde(rename = "htmlcolor")]
        Htmlcolor,
        #[serde(rename = "markdown")]
        Markdown,
        #[serde(rename = "System default")]
        SystemDefault,
    }

    impl ::std::fmt::Display for UpdateWatchNotificationFormat {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Text => f.write_str("text"),
                Self::Html => f.write_str("html"),
                Self::Htmlcolor => f.write_str("htmlcolor"),
                Self::Markdown => f.write_str("markdown"),
                Self::SystemDefault => f.write_str("System default"),
            }
        }
    }

    impl ::std::str::FromStr for UpdateWatchNotificationFormat {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "text" => Ok(Self::Text),
                "html" => Ok(Self::Html),
                "htmlcolor" => Ok(Self::Htmlcolor),
                "markdown" => Ok(Self::Markdown),
                "System default" => Ok(Self::SystemDefault),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchNotificationTitle(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchNotificationTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchNotificationTitle> for ::std::string::String {
        fn from(value: UpdateWatchNotificationTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchNotificationTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchNotificationTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatchNotificationUrlsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 1000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchNotificationUrlsItem(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchNotificationUrlsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchNotificationUrlsItem> for ::std::string::String {
        fn from(value: UpdateWatchNotificationUrlsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchNotificationUrlsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 1000usize {
                return Err("longer than 1000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchNotificationUrlsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Optional processor mode to use for change detection. Defaults to
    /// `text_json_diff` if not specified.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Optional processor mode to use for change detection.
    /// Defaults to `text_json_diff` if not specified.",
    ///  "default": "text_json_diff",
    ///  "type": "string",
    ///  "enum": [
    ///    "restock_diff",
    ///    "text_json_diff"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum UpdateWatchProcessor {
        #[serde(rename = "restock_diff")]
        RestockDiff,
        #[serde(rename = "text_json_diff")]
        TextJsonDiff,
    }

    impl ::std::fmt::Display for UpdateWatchProcessor {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::RestockDiff => f.write_str("restock_diff"),
                Self::TextJsonDiff => f.write_str("text_json_diff"),
            }
        }
    }

    impl ::std::str::FromStr for UpdateWatchProcessor {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "restock_diff" => Ok(Self::RestockDiff),
                "text_json_diff" => Ok(Self::TextJsonDiff),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchProcessor {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for UpdateWatchProcessor {
        fn default() -> Self {
            UpdateWatchProcessor::TextJsonDiff
        }
    }

    ///Proxy configuration
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Proxy configuration",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchProxy(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchProxy {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchProxy> for ::std::string::String {
        fn from(value: UpdateWatchProxy) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchProxy {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchProxy {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchProxy {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatchSubtractiveSelectorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchSubtractiveSelectorsItem(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchSubtractiveSelectorsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchSubtractiveSelectorsItem> for ::std::string::String {
        fn from(value: UpdateWatchSubtractiveSelectorsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchSubtractiveSelectorsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchSubtractiveSelectorsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Tag UUID to associate with this web page change monitor (watch)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Tag UUID to associate with this web page change monitor
    /// (watch)",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchTag(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchTag {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchTag> for ::std::string::String {
        fn from(value: UpdateWatchTag) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchTag {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchTag {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchTag {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatchTextShouldNotBePresentItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchTextShouldNotBePresentItem(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchTextShouldNotBePresentItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchTextShouldNotBePresentItem> for ::std::string::String {
        fn from(value: UpdateWatchTextShouldNotBePresentItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchTextShouldNotBePresentItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchTextShouldNotBePresentItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Time intervals between checks. All fields must be non-negative. At least
    /// one non-zero value required when not using default settings.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Time intervals between checks. All fields must be
    /// non-negative. At least one non-zero value required when not using
    /// default settings.",
    ///  "type": "object",
    ///  "properties": {
    ///    "days": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 365000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "hours": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 8760000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "minutes": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 525600000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "seconds": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 31536000000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "weeks": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 52000.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct UpdateWatchTimeBetweenCheck {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub days: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub hours: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub minutes: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub seconds: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub weeks: ::std::option::Option<i64>,
    }

    impl ::std::default::Default for UpdateWatchTimeBetweenCheck {
        fn default() -> Self {
            Self {
                days: Default::default(),
                hours: Default::default(),
                minutes: Default::default(),
                seconds: Default::default(),
                weeks: Default::default(),
            }
        }
    }

    ///Weekly schedule limiting when checks can run
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Weekly schedule limiting when checks can run",
    ///  "type": "object",
    ///  "properties": {
    ///    "enabled": {
    ///      "default": false,
    ///      "type": "boolean"
    ///    },
    ///    "friday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "monday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "saturday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "sunday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "thursday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "tuesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "wednesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct UpdateWatchTimeScheduleLimit {
        #[serde(default)]
        pub enabled: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub friday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub monday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub saturday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub sunday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub thursday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tuesday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub wednesday: ::std::option::Option<DaySchedule>,
    }

    impl ::std::default::Default for UpdateWatchTimeScheduleLimit {
        fn default() -> Self {
            Self {
                enabled: Default::default(),
                friday: Default::default(),
                monday: Default::default(),
                saturday: Default::default(),
                sunday: Default::default(),
                thursday: Default::default(),
                tuesday: Default::default(),
                wednesday: Default::default(),
            }
        }
    }

    ///Custom title for the web page change monitor (watch), not to be confused
    /// with page_title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom title for the web page change monitor (watch),
    /// not to be confused with page_title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchTitle(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchTitle> for ::std::string::String {
        fn from(value: UpdateWatchTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`UpdateWatchTriggerTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchTriggerTextItem(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchTriggerTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchTriggerTextItem> for ::std::string::String {
        fn from(value: UpdateWatchTriggerTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchTriggerTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchTriggerTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///JavaScript code to execute
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "JavaScript code to execute",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UpdateWatchWebdriverJsExecuteCode(::std::string::String);
    impl ::std::ops::Deref for UpdateWatchWebdriverJsExecuteCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UpdateWatchWebdriverJsExecuteCode> for ::std::string::String {
        fn from(value: UpdateWatchWebdriverJsExecuteCode) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UpdateWatchWebdriverJsExecuteCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UpdateWatchWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UpdateWatchWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UpdateWatchWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UpdateWatchWebdriverJsExecuteCode {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`Watch`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/WatchBase"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "browser_steps_last_error_step": {
    ///          "description": "Last browser step that caused an error",
    ///          "readOnly": true,
    ///          "type": [
    ///            "integer",
    ///            "null"
    ///          ]
    ///        },
    ///        "check_count": {
    ///          "description": "Total number of checks performed",
    ///          "readOnly": true,
    ///          "type": "integer"
    ///        },
    ///        "consecutive_filter_failures": {
    ///          "description": "Counter for consecutive filter match failures",
    ///          "readOnly": true,
    ///          "type": "integer"
    ///        },
    ///        "content-type": {
    ///          "description": "Content-Type from last fetch",
    ///          "readOnly": true,
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "fetch_time": {
    ///          "description": "Duration of last fetch in seconds",
    ///          "readOnly": true,
    ///          "type": "number"
    ///        },
    ///        "history_n": {
    ///          "description": "Number of history snapshots available",
    ///          "readOnly": true,
    ///          "type": "integer",
    ///          "x-computed": true
    ///        },
    ///        "last_changed": {
    ///          "description": "Unix timestamp of last change",
    ///          "readOnly": true,
    ///          "type": "integer",
    ///          "x-computed": true
    ///        },
    ///        "last_checked": {
    ///          "description": "Unix timestamp of last check",
    ///          "readOnly": true,
    ///          "type": "integer"
    ///        },
    ///        "last_error": {
    ///          "description": "Last error message (false when no error, string
    /// when error occurred, null if not checked yet)",
    ///          "readOnly": true,
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "last_notification_error": {
    ///          "description": "Last notification error message",
    ///          "readOnly": true,
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "last_viewed": {
    ///          "description": "Unix timestamp in seconds of the last time the
    /// watch was viewed. Setting it to a value higher than `last_changed` in
    /// the \"Update watch\" endpoint marks the watch as viewed.",
    ///          "type": "integer",
    ///          "minimum": 0.0
    ///        },
    ///        "link": {
    ///          "description": "The watch URL rendered in case of any Jinja2
    /// markup, always use this for listing.",
    ///          "readOnly": true,
    ///          "type": "string",
    ///          "format": "string",
    ///          "x-computed": true
    ///        },
    ///        "notification_alert_count": {
    ///          "description": "Number of notifications sent",
    ///          "readOnly": true,
    ///          "type": "integer"
    ///        },
    ///        "page_title": {
    ///          "description": "HTML <title> tag extracted from the page",
    ///          "readOnly": true,
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "previous_md5": {
    ///          "description": "MD5 hash of previous content (false if not
    /// set)",
    ///          "readOnly": true,
    ///          "type": "string"
    ///        },
    ///        "previous_md5_before_filters": {
    ///          "description": "MD5 hash before filters applied (false if not
    /// set)",
    ///          "readOnly": true,
    ///          "type": "string"
    ///        },
    ///        "processor_config_restock_diff": {
    ///          "description": "Resolved restock/price processor config for
    /// this watch.\nIf a tag with `overrides_watch: true` is assigned to this
    /// watch, the tag's config is\nreturned here instead of the watch's own
    /// config. Use `processor_config_restock_diff_source`\nto determine where
    /// the config originated.\n",
    ///          "readOnly": true,
    ///          "type": "object",
    ///          "properties": {
    ///            "follow_price_changes": {
    ///              "type": "boolean"
    ///            },
    ///            "in_stock_processing": {
    ///              "type": "string",
    ///              "enum": [
    ///                "in_stock_only",
    ///                "all_changes",
    ///                "off"
    ///              ]
    ///            },
    ///            "price_change_max": {
    ///              "type": [
    ///                "number",
    ///                "null"
    ///              ]
    ///            },
    ///            "price_change_min": {
    ///              "type": [
    ///                "number",
    ///                "null"
    ///              ]
    ///            },
    ///            "price_change_threshold_percent": {
    ///              "type": [
    ///                "number",
    ///                "null"
    ///              ],
    ///              "maximum": 100.0,
    ///              "minimum": 0.0
    ///            }
    ///          },
    ///          "x-computed": true
    ///        },
    ///        "processor_config_restock_diff_source": {
    ///          "description": "Indicates the origin of
    /// `processor_config_restock_diff`.\n- `watch`: config comes from the watch
    /// itself\n- `tag:<uuid>`: config is overridden by the tag with the given
    /// UUID\n",
    ///          "readOnly": true,
    ///          "type": "string",
    ///          "x-computed": true
    ///        },
    ///        "remote_server_reply": {
    ///          "description": "Server header from last response",
    ///          "readOnly": true,
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "viewed": {
    ///          "description": "Computed property - true if watch has been
    /// viewed, false otherwise (deprecated, use last_viewed instead)",
    ///          "readOnly": true,
    ///          "type": "integer",
    ///          "x-computed": true
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct Watch {
        ///HTTP request body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub body: ::std::option::Option<WatchBody>,
        ///Browser automation steps. Maximum 100 steps allowed.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub browser_steps: ::std::vec::Vec<WatchBrowserStepsItem>,
        ///Last browser step that caused an error
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub browser_steps_last_error_step: ::std::option::Option<i64>,
        ///Total number of checks performed
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub check_count: ::std::option::Option<i64>,
        ///Compare against all history for unique lines
        #[serde(default)]
        pub check_unique_lines: bool,
        ///Array of condition rules for change detection logic (empty array
        /// when not set)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub conditions: ::std::vec::Vec<WatchConditionsItem>,
        ///Logic operator - ALL (match all conditions) or ANY (match any
        /// condition)
        #[serde(default = "defaults::watch_conditions_match_logic")]
        pub conditions_match_logic: WatchConditionsMatchLogic,
        ///Counter for consecutive filter match failures
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub consecutive_filter_failures: ::std::option::Option<i64>,
        ///Content-Type from last fetch
        #[serde(
            rename = "content-type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub content_type: ::std::option::Option<::std::string::String>,
        ///Unix timestamp of creation
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub date_created: ::std::option::Option<i64>,
        ///Keep only lines containing these substrings (plain text,
        /// case-insensitive) — simpler alternative to regex
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_lines_containing: ::std::vec::Vec<WatchExtractLinesContainingItem>,
        ///Regex patterns to extract specific text after filtering
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_text: ::std::vec::Vec<WatchExtractTextItem>,
        ///Backend to use for fetching content. Common values:
        /// - `system` (default) - Use the system-wide default fetcher
        /// - `html_requests` - Fast requests-based fetcher
        /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
        /// - `extra_browser_*` - Custom browser configurations (if configured)
        /// - Plugin-provided fetchers (if installed)
        #[serde(default = "defaults::watch_fetch_backend")]
        pub fetch_backend: WatchFetchBackend,
        ///Duration of last fetch in seconds
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub fetch_time: ::std::option::Option<f64>,
        ///Send notification when filters fail to match content
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_failure_notification_send: bool,
        ///Include added text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_added: bool,
        ///Include removed text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_removed: bool,
        ///Include replaced text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_replaced: bool,
        ///Monitor and track price changes (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub follow_price_changes: bool,
        ///Whether page has LD-JSON price data (auto-detected)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub has_ldjson_price_data: ::std::option::Option<bool>,
        ///HTTP headers to include in requests
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub headers: ::std::collections::HashMap<::std::string::String, ::std::string::String>,
        ///Number of history snapshots available
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub history_n: ::std::option::Option<i64>,
        ///Maximum number of history snapshots to keep (null = use system
        /// default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub history_snapshot_max_length: ::std::option::Option<::std::num::NonZeroU64>,
        ///Ignore HTTP status code errors (boolean or null)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ignore_status_codes: ::std::option::Option<bool>,
        ///Text patterns to ignore in change detection
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub ignore_text: ::std::vec::Vec<WatchIgnoreTextItem>,
        ///Only trigger on in-stock transitions (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub in_stock_only: bool,
        ///CSS/XPath selectors to extract specific content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub include_filters: ::std::vec::Vec<WatchIncludeFiltersItem>,
        ///Unix timestamp of last change
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub last_changed: ::std::option::Option<i64>,
        ///Unix timestamp of last check
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub last_checked: ::std::option::Option<i64>,
        ///Last error message (false when no error, string when error occurred,
        /// null if not checked yet)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub last_error: ::std::option::Option<::std::string::String>,
        ///Last notification error message
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub last_notification_error: ::std::option::Option<::std::string::String>,
        ///Unix timestamp in seconds of the last time the watch was viewed.
        /// Setting it to a value higher than `last_changed` in the "Update
        /// watch" endpoint marks the watch as viewed.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub last_viewed: ::std::option::Option<u64>,
        ///The watch URL rendered in case of any Jinja2 markup, always use this
        /// for listing.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub link: ::std::option::Option<::std::string::String>,
        ///Internal cache of AI evaluation results keyed by (intent, diff) hash
        /// (auto-managed).
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub llm_evaluation_cache: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        ///Number of tokens consumed by the AI on the most recent check.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_last_tokens_used: ::std::option::Option<i64>,
        ///CSS selector derived by the AI to narrow content scope before
        /// evaluation (auto-managed, do not set manually).
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_prefilter: ::std::option::Option<::std::string::String>,
        ///Total tokens consumed by the AI across all checks for this watch.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_tokens_used_cumulative: ::std::option::Option<i64>,
        ///HTTP method to use
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub method: ::std::option::Option<WatchMethod>,
        ///Number of notifications sent
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_alert_count: ::std::option::Option<i64>,
        ///Custom notification body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_body: ::std::option::Option<WatchNotificationBody>,
        ///Format for notifications
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_format: ::std::option::Option<WatchNotificationFormat>,
        ///Whether notifications are muted
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_muted: ::std::option::Option<bool>,
        ///Include screenshot in notifications (if supported by notification
        /// URL)
        #[serde(default)]
        pub notification_screenshot: bool,
        ///Custom notification title
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_title: ::std::option::Option<WatchNotificationTitle>,
        ///Notification URLs for this web page change monitor (watch). Maximum
        /// 100 URLs.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub notification_urls: ::std::vec::Vec<WatchNotificationUrlsItem>,
        ///HTML <title> tag extracted from the page
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub page_title: ::std::option::Option<::std::string::String>,
        ///Whether the web page change monitor (watch) is paused
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub paused: ::std::option::Option<bool>,
        ///MD5 hash of previous content (false if not set)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub previous_md5: ::std::option::Option<::std::string::String>,
        ///MD5 hash before filters applied (false if not set)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub previous_md5_before_filters: ::std::option::Option<::std::string::String>,
        ///Minimum price change percentage to trigger notification
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_change_threshold_percent: ::std::option::Option<f64>,
        ///Optional processor mode to use for change detection. Defaults to
        /// `text_json_diff` if not specified.
        #[serde(default = "defaults::watch_processor")]
        pub processor: WatchProcessor,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub processor_config_restock_diff: ::std::option::Option<WatchProcessorConfigRestockDiff>,
        ///Indicates the origin of `processor_config_restock_diff`.
        /// - `watch`: config comes from the watch itself
        /// - `tag:<uuid>`: config is overridden by the tag with the given UUID
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub processor_config_restock_diff_source: ::std::option::Option<::std::string::String>,
        ///Proxy configuration
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxy: ::std::option::Option<WatchProxy>,
        ///Server header from last response
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub remote_server_reply: ::std::option::Option<::std::string::String>,
        ///Remove duplicate lines from content
        #[serde(default)]
        pub remove_duplicate_lines: bool,
        ///Sort lines alphabetically before comparison
        #[serde(default)]
        pub sort_text_alphabetically: bool,
        ///Remove lines matching ignore patterns
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub strip_ignored_lines: ::std::option::Option<bool>,
        ///CSS/XPath selectors to remove content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub subtractive_selectors: ::std::vec::Vec<WatchSubtractiveSelectorsItem>,
        ///Tag UUID to associate with this web page change monitor (watch)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tag: ::std::option::Option<WatchTag>,
        ///Array of tag UUIDs
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///Text that should NOT be present (triggers alert if found)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub text_should_not_be_present: ::std::vec::Vec<WatchTextShouldNotBePresentItem>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_between_check: ::std::option::Option<WatchTimeBetweenCheck>,
        ///Whether to use global settings for time between checks - defaults to
        /// true if not set
        #[serde(default = "defaults::default_bool::<true>")]
        pub time_between_check_use_default: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_schedule_limit: ::std::option::Option<WatchTimeScheduleLimit>,
        ///Custom title for the web page change monitor (watch), not to be
        /// confused with page_title
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub title: ::std::option::Option<WatchTitle>,
        ///Whether to track JSON-LD price data
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub track_ldjson_price_data: ::std::option::Option<bool>,
        ///Text/regex patterns that must be present to trigger a change
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub trigger_text: ::std::vec::Vec<WatchTriggerTextItem>,
        ///Strip leading/trailing whitespace from text
        #[serde(default)]
        pub trim_text_whitespace: bool,
        ///URL to monitor for changes
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub url: ::std::option::Option<::std::string::String>,
        ///Display page title in watch list (null = use system default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub use_page_title_in_list: ::std::option::Option<bool>,
        ///Unique identifier
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub uuid: ::std::option::Option<::uuid::Uuid>,
        ///Computed property - true if watch has been viewed, false otherwise
        /// (deprecated, use last_viewed instead)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub viewed: ::std::option::Option<i64>,
        ///Delay in seconds for webdriver
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_delay: ::std::option::Option<i64>,
        ///JavaScript code to execute
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_js_execute_code: ::std::option::Option<WatchWebdriverJsExecuteCode>,
    }

    impl ::std::default::Default for Watch {
        fn default() -> Self {
            Self {
                body: Default::default(),
                browser_steps: Default::default(),
                browser_steps_last_error_step: Default::default(),
                check_count: Default::default(),
                check_unique_lines: Default::default(),
                conditions: Default::default(),
                conditions_match_logic: defaults::watch_conditions_match_logic(),
                consecutive_filter_failures: Default::default(),
                content_type: Default::default(),
                date_created: Default::default(),
                extract_lines_containing: Default::default(),
                extract_text: Default::default(),
                fetch_backend: defaults::watch_fetch_backend(),
                fetch_time: Default::default(),
                filter_failure_notification_send: defaults::default_bool::<true>(),
                filter_text_added: defaults::default_bool::<true>(),
                filter_text_removed: defaults::default_bool::<true>(),
                filter_text_replaced: defaults::default_bool::<true>(),
                follow_price_changes: defaults::default_bool::<true>(),
                has_ldjson_price_data: Default::default(),
                headers: Default::default(),
                history_n: Default::default(),
                history_snapshot_max_length: Default::default(),
                ignore_status_codes: Default::default(),
                ignore_text: Default::default(),
                in_stock_only: defaults::default_bool::<true>(),
                include_filters: Default::default(),
                last_changed: Default::default(),
                last_checked: Default::default(),
                last_error: Default::default(),
                last_notification_error: Default::default(),
                last_viewed: Default::default(),
                link: Default::default(),
                llm_evaluation_cache: Default::default(),
                llm_last_tokens_used: Default::default(),
                llm_prefilter: Default::default(),
                llm_tokens_used_cumulative: Default::default(),
                method: Default::default(),
                notification_alert_count: Default::default(),
                notification_body: Default::default(),
                notification_format: Default::default(),
                notification_muted: Default::default(),
                notification_screenshot: Default::default(),
                notification_title: Default::default(),
                notification_urls: Default::default(),
                page_title: Default::default(),
                paused: Default::default(),
                previous_md5: Default::default(),
                previous_md5_before_filters: Default::default(),
                price_change_threshold_percent: Default::default(),
                processor: defaults::watch_processor(),
                processor_config_restock_diff: Default::default(),
                processor_config_restock_diff_source: Default::default(),
                proxy: Default::default(),
                remote_server_reply: Default::default(),
                remove_duplicate_lines: Default::default(),
                sort_text_alphabetically: Default::default(),
                strip_ignored_lines: Default::default(),
                subtractive_selectors: Default::default(),
                tag: Default::default(),
                tags: Default::default(),
                text_should_not_be_present: Default::default(),
                time_between_check: Default::default(),
                time_between_check_use_default: defaults::default_bool::<true>(),
                time_schedule_limit: Default::default(),
                title: Default::default(),
                track_ldjson_price_data: Default::default(),
                trigger_text: Default::default(),
                trim_text_whitespace: Default::default(),
                url: Default::default(),
                use_page_title_in_list: Default::default(),
                uuid: Default::default(),
                viewed: Default::default(),
                webdriver_delay: Default::default(),
                webdriver_js_execute_code: Default::default(),
            }
        }
    }

    ///`WatchBase`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "body": {
    ///      "description": "HTTP request body",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "browser_steps": {
    ///      "description": "Browser automation steps. Maximum 100 steps
    /// allowed.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "operation",
    ///          "optional_value",
    ///          "selector"
    ///        ],
    ///        "properties": {
    ///          "operation": {
    ///            "type": [
    ///              "string",
    ///              "null"
    ///            ],
    ///            "maxLength": 5000
    ///          },
    ///          "optional_value": {
    ///            "type": [
    ///              "string",
    ///              "null"
    ///            ],
    ///            "maxLength": 5000
    ///          },
    ///          "selector": {
    ///            "type": [
    ///              "string",
    ///              "null"
    ///            ],
    ///            "maxLength": 5000
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "check_unique_lines": {
    ///      "description": "Compare against all history for unique lines",
    ///      "default": false,
    ///      "type": "boolean"
    ///    },
    ///    "conditions": {
    ///      "description": "Array of condition rules for change detection logic
    /// (empty array when not set)",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "field",
    ///          "operator",
    ///          "value"
    ///        ],
    ///        "properties": {
    ///          "field": {
    ///            "description": "Field to check (e.g., 'page_filtered_text',
    /// 'page_title')",
    ///            "type": "string"
    ///          },
    ///          "operator": {
    ///            "description": "Comparison operator (e.g., 'contains_regex',
    /// 'equals', 'not_equals')",
    ///            "type": "string"
    ///          },
    ///          "value": {
    ///            "description": "Value to compare against",
    ///            "type": "string"
    ///          }
    ///        }
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "conditions_match_logic": {
    ///      "description": "Logic operator - ALL (match all conditions) or ANY
    /// (match any condition)",
    ///      "default": "ALL",
    ///      "type": "string",
    ///      "enum": [
    ///        "ALL",
    ///        "ANY"
    ///      ]
    ///    },
    ///    "date_created": {
    ///      "description": "Unix timestamp of creation",
    ///      "readOnly": true,
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ]
    ///    },
    ///    "extract_lines_containing": {
    ///      "description": "Keep only lines containing these substrings (plain
    /// text, case-insensitive) — simpler alternative to regex",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "maxLength": 5000
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "extract_text": {
    ///      "description": "Regex patterns to extract specific text after
    /// filtering",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "maxLength": 5000
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "fetch_backend": {
    ///      "description": "Backend to use for fetching content. Common
    /// values:\n- `system` (default) - Use the system-wide default fetcher\n-
    /// `html_requests` - Fast requests-based fetcher\n- `html_webdriver` -
    /// Browser-based fetcher (Playwright/Puppeteer)\n- `extra_browser_*` -
    /// Custom browser configurations (if configured)\n- Plugin-provided
    /// fetchers (if installed)\n",
    ///      "default": "system",
    ///      "type": "string",
    ///      "pattern":
    /// "^(system|html_requests|html_webdriver|extra_browser_.+)$"
    ///    },
    ///    "filter_failure_notification_send": {
    ///      "description": "Send notification when filters fail to match
    /// content",
    ///      "default": true,
    ///      "type": "boolean"
    ///    },
    ///    "filter_text_added": {
    ///      "description": "Include added text in change detection",
    ///      "default": true,
    ///      "type": "boolean"
    ///    },
    ///    "filter_text_removed": {
    ///      "description": "Include removed text in change detection",
    ///      "default": true,
    ///      "type": "boolean"
    ///    },
    ///    "filter_text_replaced": {
    ///      "description": "Include replaced text in change detection",
    ///      "default": true,
    ///      "type": "boolean"
    ///    },
    ///    "follow_price_changes": {
    ///      "description": "Monitor and track price changes (restock_diff
    /// processor)",
    ///      "default": true,
    ///      "type": "boolean"
    ///    },
    ///    "has_ldjson_price_data": {
    ///      "description": "Whether page has LD-JSON price data
    /// (auto-detected)",
    ///      "readOnly": true,
    ///      "type": [
    ///        "boolean",
    ///        "null"
    ///      ]
    ///    },
    ///    "headers": {
    ///      "description": "HTTP headers to include in requests",
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "history_snapshot_max_length": {
    ///      "description": "Maximum number of history snapshots to keep (null =
    /// use system default)",
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 1000.0,
    ///      "minimum": 1.0
    ///    },
    ///    "ignore_status_codes": {
    ///      "description": "Ignore HTTP status code errors (boolean or null)",
    ///      "type": [
    ///        "boolean",
    ///        "null"
    ///      ]
    ///    },
    ///    "ignore_text": {
    ///      "description": "Text patterns to ignore in change detection",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "maxLength": 5000
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "in_stock_only": {
    ///      "description": "Only trigger on in-stock transitions (restock_diff
    /// processor)",
    ///      "default": true,
    ///      "type": "boolean"
    ///    },
    ///    "include_filters": {
    ///      "description": "CSS/XPath selectors to extract specific content
    /// from the page",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "maxLength": 5000
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "llm_evaluation_cache": {
    ///      "description": "Internal cache of AI evaluation results keyed by
    /// (intent, diff) hash (auto-managed).",
    ///      "readOnly": true,
    ///      "type": "object"
    ///    },
    ///    "llm_last_tokens_used": {
    ///      "description": "Number of tokens consumed by the AI on the most
    /// recent check.",
    ///      "readOnly": true,
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ]
    ///    },
    ///    "llm_prefilter": {
    ///      "description": "CSS selector derived by the AI to narrow content
    /// scope before evaluation (auto-managed, do not set manually).",
    ///      "readOnly": true,
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "llm_tokens_used_cumulative": {
    ///      "description": "Total tokens consumed by the AI across all checks
    /// for this watch.",
    ///      "readOnly": true,
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ]
    ///    },
    ///    "method": {
    ///      "description": "HTTP method to use",
    ///      "type": "string",
    ///      "enum": [
    ///        "GET",
    ///        "POST",
    ///        "DELETE",
    ///        "PUT"
    ///      ]
    ///    },
    ///    "notification_body": {
    ///      "description": "Custom notification body",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "notification_format": {
    ///      "description": "Format for notifications",
    ///      "type": "string",
    ///      "enum": [
    ///        "text",
    ///        "html",
    ///        "htmlcolor",
    ///        "markdown",
    ///        "System default"
    ///      ]
    ///    },
    ///    "notification_muted": {
    ///      "description": "Whether notifications are muted",
    ///      "type": "boolean"
    ///    },
    ///    "notification_screenshot": {
    ///      "description": "Include screenshot in notifications (if supported
    /// by notification URL)",
    ///      "default": false,
    ///      "type": "boolean"
    ///    },
    ///    "notification_title": {
    ///      "description": "Custom notification title",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "notification_urls": {
    ///      "description": "Notification URLs for this web page change monitor
    /// (watch). Maximum 100 URLs.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "maxLength": 1000
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "paused": {
    ///      "description": "Whether the web page change monitor (watch) is
    /// paused",
    ///      "type": "boolean"
    ///    },
    ///    "price_change_threshold_percent": {
    ///      "description": "Minimum price change percentage to trigger
    /// notification",
    ///      "type": [
    ///        "number",
    ///        "null"
    ///      ]
    ///    },
    ///    "processor": {
    ///      "description": "Optional processor mode to use for change
    /// detection. Defaults to `text_json_diff` if not specified.",
    ///      "default": "text_json_diff",
    ///      "type": "string",
    ///      "enum": [
    ///        "restock_diff",
    ///        "text_json_diff"
    ///      ]
    ///    },
    ///    "proxy": {
    ///      "description": "Proxy configuration",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "remove_duplicate_lines": {
    ///      "description": "Remove duplicate lines from content",
    ///      "default": false,
    ///      "type": "boolean"
    ///    },
    ///    "sort_text_alphabetically": {
    ///      "description": "Sort lines alphabetically before comparison",
    ///      "default": false,
    ///      "type": "boolean"
    ///    },
    ///    "strip_ignored_lines": {
    ///      "description": "Remove lines matching ignore patterns",
    ///      "type": [
    ///        "boolean",
    ///        "null"
    ///      ]
    ///    },
    ///    "subtractive_selectors": {
    ///      "description": "CSS/XPath selectors to remove content from the
    /// page",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "maxLength": 5000
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "tag": {
    ///      "description": "Tag UUID to associate with this web page change
    /// monitor (watch)",
    ///      "type": "string",
    ///      "maxLength": 5000
    ///    },
    ///    "tags": {
    ///      "description": "Array of tag UUIDs",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "text_should_not_be_present": {
    ///      "description": "Text that should NOT be present (triggers alert if
    /// found)",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "maxLength": 5000
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "time_between_check": {
    ///      "description": "Time intervals between checks. All fields must be
    /// non-negative. At least one non-zero value required when not using
    /// default settings.",
    ///      "type": "object",
    ///      "properties": {
    ///        "days": {
    ///          "type": [
    ///            "integer",
    ///            "null"
    ///          ],
    ///          "maximum": 365000.0,
    ///          "minimum": 0.0
    ///        },
    ///        "hours": {
    ///          "type": [
    ///            "integer",
    ///            "null"
    ///          ],
    ///          "maximum": 8760000.0,
    ///          "minimum": 0.0
    ///        },
    ///        "minutes": {
    ///          "type": [
    ///            "integer",
    ///            "null"
    ///          ],
    ///          "maximum": 525600000.0,
    ///          "minimum": 0.0
    ///        },
    ///        "seconds": {
    ///          "type": [
    ///            "integer",
    ///            "null"
    ///          ],
    ///          "maximum": 31536000000.0,
    ///          "minimum": 0.0
    ///        },
    ///        "weeks": {
    ///          "type": [
    ///            "integer",
    ///            "null"
    ///          ],
    ///          "maximum": 52000.0,
    ///          "minimum": 0.0
    ///        }
    ///      }
    ///    },
    ///    "time_between_check_use_default": {
    ///      "description": "Whether to use global settings for time between
    /// checks - defaults to true if not set",
    ///      "default": true,
    ///      "type": "boolean"
    ///    },
    ///    "time_schedule_limit": {
    ///      "description": "Weekly schedule limiting when checks can run",
    ///      "type": "object",
    ///      "properties": {
    ///        "enabled": {
    ///          "default": false,
    ///          "type": "boolean"
    ///        },
    ///        "friday": {
    ///          "$ref": "#/components/schemas/DaySchedule"
    ///        },
    ///        "monday": {
    ///          "$ref": "#/components/schemas/DaySchedule"
    ///        },
    ///        "saturday": {
    ///          "$ref": "#/components/schemas/DaySchedule"
    ///        },
    ///        "sunday": {
    ///          "$ref": "#/components/schemas/DaySchedule"
    ///        },
    ///        "thursday": {
    ///          "$ref": "#/components/schemas/DaySchedule"
    ///        },
    ///        "tuesday": {
    ///          "$ref": "#/components/schemas/DaySchedule"
    ///        },
    ///        "wednesday": {
    ///          "$ref": "#/components/schemas/DaySchedule"
    ///        }
    ///      }
    ///    },
    ///    "title": {
    ///      "description": "Custom title for the web page change monitor
    /// (watch), not to be confused with page_title",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "track_ldjson_price_data": {
    ///      "description": "Whether to track JSON-LD price data",
    ///      "type": [
    ///        "boolean",
    ///        "null"
    ///      ]
    ///    },
    ///    "trigger_text": {
    ///      "description": "Text/regex patterns that must be present to trigger
    /// a change",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "maxLength": 5000
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "trim_text_whitespace": {
    ///      "description": "Strip leading/trailing whitespace from text",
    ///      "default": false,
    ///      "type": "boolean"
    ///    },
    ///    "url": {
    ///      "description": "URL to monitor for changes",
    ///      "type": "string",
    ///      "format": "uri",
    ///      "maxLength": 5000
    ///    },
    ///    "use_page_title_in_list": {
    ///      "description": "Display page title in watch list (null = use system
    /// default)",
    ///      "type": [
    ///        "boolean",
    ///        "null"
    ///      ]
    ///    },
    ///    "uuid": {
    ///      "description": "Unique identifier",
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "format": "uuid"
    ///    },
    ///    "webdriver_delay": {
    ///      "description": "Delay in seconds for webdriver",
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ]
    ///    },
    ///    "webdriver_js_execute_code": {
    ///      "description": "JavaScript code to execute",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct WatchBase {
        ///HTTP request body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub body: ::std::option::Option<WatchBaseBody>,
        ///Browser automation steps. Maximum 100 steps allowed.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub browser_steps: ::std::vec::Vec<WatchBaseBrowserStepsItem>,
        ///Compare against all history for unique lines
        #[serde(default)]
        pub check_unique_lines: bool,
        ///Array of condition rules for change detection logic (empty array
        /// when not set)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub conditions: ::std::vec::Vec<WatchBaseConditionsItem>,
        ///Logic operator - ALL (match all conditions) or ANY (match any
        /// condition)
        #[serde(default = "defaults::watch_base_conditions_match_logic")]
        pub conditions_match_logic: WatchBaseConditionsMatchLogic,
        ///Unix timestamp of creation
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub date_created: ::std::option::Option<i64>,
        ///Keep only lines containing these substrings (plain text,
        /// case-insensitive) — simpler alternative to regex
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_lines_containing: ::std::vec::Vec<WatchBaseExtractLinesContainingItem>,
        ///Regex patterns to extract specific text after filtering
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub extract_text: ::std::vec::Vec<WatchBaseExtractTextItem>,
        ///Backend to use for fetching content. Common values:
        /// - `system` (default) - Use the system-wide default fetcher
        /// - `html_requests` - Fast requests-based fetcher
        /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
        /// - `extra_browser_*` - Custom browser configurations (if configured)
        /// - Plugin-provided fetchers (if installed)
        #[serde(default = "defaults::watch_base_fetch_backend")]
        pub fetch_backend: WatchBaseFetchBackend,
        ///Send notification when filters fail to match content
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_failure_notification_send: bool,
        ///Include added text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_added: bool,
        ///Include removed text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_removed: bool,
        ///Include replaced text in change detection
        #[serde(default = "defaults::default_bool::<true>")]
        pub filter_text_replaced: bool,
        ///Monitor and track price changes (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub follow_price_changes: bool,
        ///Whether page has LD-JSON price data (auto-detected)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub has_ldjson_price_data: ::std::option::Option<bool>,
        ///HTTP headers to include in requests
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub headers: ::std::collections::HashMap<::std::string::String, ::std::string::String>,
        ///Maximum number of history snapshots to keep (null = use system
        /// default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub history_snapshot_max_length: ::std::option::Option<::std::num::NonZeroU64>,
        ///Ignore HTTP status code errors (boolean or null)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ignore_status_codes: ::std::option::Option<bool>,
        ///Text patterns to ignore in change detection
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub ignore_text: ::std::vec::Vec<WatchBaseIgnoreTextItem>,
        ///Only trigger on in-stock transitions (restock_diff processor)
        #[serde(default = "defaults::default_bool::<true>")]
        pub in_stock_only: bool,
        ///CSS/XPath selectors to extract specific content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub include_filters: ::std::vec::Vec<WatchBaseIncludeFiltersItem>,
        ///Internal cache of AI evaluation results keyed by (intent, diff) hash
        /// (auto-managed).
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub llm_evaluation_cache: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        ///Number of tokens consumed by the AI on the most recent check.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_last_tokens_used: ::std::option::Option<i64>,
        ///CSS selector derived by the AI to narrow content scope before
        /// evaluation (auto-managed, do not set manually).
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_prefilter: ::std::option::Option<::std::string::String>,
        ///Total tokens consumed by the AI across all checks for this watch.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub llm_tokens_used_cumulative: ::std::option::Option<i64>,
        ///HTTP method to use
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub method: ::std::option::Option<WatchBaseMethod>,
        ///Custom notification body
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_body: ::std::option::Option<WatchBaseNotificationBody>,
        ///Format for notifications
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_format: ::std::option::Option<WatchBaseNotificationFormat>,
        ///Whether notifications are muted
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_muted: ::std::option::Option<bool>,
        ///Include screenshot in notifications (if supported by notification
        /// URL)
        #[serde(default)]
        pub notification_screenshot: bool,
        ///Custom notification title
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notification_title: ::std::option::Option<WatchBaseNotificationTitle>,
        ///Notification URLs for this web page change monitor (watch). Maximum
        /// 100 URLs.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub notification_urls: ::std::vec::Vec<WatchBaseNotificationUrlsItem>,
        ///Whether the web page change monitor (watch) is paused
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub paused: ::std::option::Option<bool>,
        ///Minimum price change percentage to trigger notification
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_change_threshold_percent: ::std::option::Option<f64>,
        ///Optional processor mode to use for change detection. Defaults to
        /// `text_json_diff` if not specified.
        #[serde(default = "defaults::watch_base_processor")]
        pub processor: WatchBaseProcessor,
        ///Proxy configuration
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxy: ::std::option::Option<WatchBaseProxy>,
        ///Remove duplicate lines from content
        #[serde(default)]
        pub remove_duplicate_lines: bool,
        ///Sort lines alphabetically before comparison
        #[serde(default)]
        pub sort_text_alphabetically: bool,
        ///Remove lines matching ignore patterns
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub strip_ignored_lines: ::std::option::Option<bool>,
        ///CSS/XPath selectors to remove content from the page
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub subtractive_selectors: ::std::vec::Vec<WatchBaseSubtractiveSelectorsItem>,
        ///Tag UUID to associate with this web page change monitor (watch)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tag: ::std::option::Option<WatchBaseTag>,
        ///Array of tag UUIDs
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///Text that should NOT be present (triggers alert if found)
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub text_should_not_be_present: ::std::vec::Vec<WatchBaseTextShouldNotBePresentItem>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_between_check: ::std::option::Option<WatchBaseTimeBetweenCheck>,
        ///Whether to use global settings for time between checks - defaults to
        /// true if not set
        #[serde(default = "defaults::default_bool::<true>")]
        pub time_between_check_use_default: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub time_schedule_limit: ::std::option::Option<WatchBaseTimeScheduleLimit>,
        ///Custom title for the web page change monitor (watch), not to be
        /// confused with page_title
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub title: ::std::option::Option<WatchBaseTitle>,
        ///Whether to track JSON-LD price data
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub track_ldjson_price_data: ::std::option::Option<bool>,
        ///Text/regex patterns that must be present to trigger a change
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub trigger_text: ::std::vec::Vec<WatchBaseTriggerTextItem>,
        ///Strip leading/trailing whitespace from text
        #[serde(default)]
        pub trim_text_whitespace: bool,
        ///URL to monitor for changes
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub url: ::std::option::Option<::std::string::String>,
        ///Display page title in watch list (null = use system default)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub use_page_title_in_list: ::std::option::Option<bool>,
        ///Unique identifier
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub uuid: ::std::option::Option<::uuid::Uuid>,
        ///Delay in seconds for webdriver
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_delay: ::std::option::Option<i64>,
        ///JavaScript code to execute
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub webdriver_js_execute_code: ::std::option::Option<WatchBaseWebdriverJsExecuteCode>,
    }

    impl ::std::default::Default for WatchBase {
        fn default() -> Self {
            Self {
                body: Default::default(),
                browser_steps: Default::default(),
                check_unique_lines: Default::default(),
                conditions: Default::default(),
                conditions_match_logic: defaults::watch_base_conditions_match_logic(),
                date_created: Default::default(),
                extract_lines_containing: Default::default(),
                extract_text: Default::default(),
                fetch_backend: defaults::watch_base_fetch_backend(),
                filter_failure_notification_send: defaults::default_bool::<true>(),
                filter_text_added: defaults::default_bool::<true>(),
                filter_text_removed: defaults::default_bool::<true>(),
                filter_text_replaced: defaults::default_bool::<true>(),
                follow_price_changes: defaults::default_bool::<true>(),
                has_ldjson_price_data: Default::default(),
                headers: Default::default(),
                history_snapshot_max_length: Default::default(),
                ignore_status_codes: Default::default(),
                ignore_text: Default::default(),
                in_stock_only: defaults::default_bool::<true>(),
                include_filters: Default::default(),
                llm_evaluation_cache: Default::default(),
                llm_last_tokens_used: Default::default(),
                llm_prefilter: Default::default(),
                llm_tokens_used_cumulative: Default::default(),
                method: Default::default(),
                notification_body: Default::default(),
                notification_format: Default::default(),
                notification_muted: Default::default(),
                notification_screenshot: Default::default(),
                notification_title: Default::default(),
                notification_urls: Default::default(),
                paused: Default::default(),
                price_change_threshold_percent: Default::default(),
                processor: defaults::watch_base_processor(),
                proxy: Default::default(),
                remove_duplicate_lines: Default::default(),
                sort_text_alphabetically: Default::default(),
                strip_ignored_lines: Default::default(),
                subtractive_selectors: Default::default(),
                tag: Default::default(),
                tags: Default::default(),
                text_should_not_be_present: Default::default(),
                time_between_check: Default::default(),
                time_between_check_use_default: defaults::default_bool::<true>(),
                time_schedule_limit: Default::default(),
                title: Default::default(),
                track_ldjson_price_data: Default::default(),
                trigger_text: Default::default(),
                trim_text_whitespace: Default::default(),
                url: Default::default(),
                use_page_title_in_list: Default::default(),
                uuid: Default::default(),
                webdriver_delay: Default::default(),
                webdriver_js_execute_code: Default::default(),
            }
        }
    }

    ///HTTP request body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP request body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseBody(::std::string::String);
    impl ::std::ops::Deref for WatchBaseBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseBody> for ::std::string::String {
        fn from(value: WatchBaseBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBaseBrowserStepsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "operation",
    ///    "optional_value",
    ///    "selector"
    ///  ],
    ///  "properties": {
    ///    "operation": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "optional_value": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "selector": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct WatchBaseBrowserStepsItem {
        pub operation: ::std::option::Option<WatchBaseBrowserStepsItemOperation>,
        pub optional_value: ::std::option::Option<WatchBaseBrowserStepsItemOptionalValue>,
        pub selector: ::std::option::Option<WatchBaseBrowserStepsItemSelector>,
    }

    ///`WatchBaseBrowserStepsItemOperation`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseBrowserStepsItemOperation(::std::string::String);
    impl ::std::ops::Deref for WatchBaseBrowserStepsItemOperation {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseBrowserStepsItemOperation> for ::std::string::String {
        fn from(value: WatchBaseBrowserStepsItemOperation) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseBrowserStepsItemOperation {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseBrowserStepsItemOperation {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBaseBrowserStepsItemOptionalValue`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseBrowserStepsItemOptionalValue(::std::string::String);
    impl ::std::ops::Deref for WatchBaseBrowserStepsItemOptionalValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseBrowserStepsItemOptionalValue> for ::std::string::String {
        fn from(value: WatchBaseBrowserStepsItemOptionalValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseBrowserStepsItemOptionalValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseBrowserStepsItemOptionalValue {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBaseBrowserStepsItemSelector`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseBrowserStepsItemSelector(::std::string::String);
    impl ::std::ops::Deref for WatchBaseBrowserStepsItemSelector {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseBrowserStepsItemSelector> for ::std::string::String {
        fn from(value: WatchBaseBrowserStepsItemSelector) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseBrowserStepsItemSelector {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseBrowserStepsItemSelector {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBaseConditionsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "field",
    ///    "operator",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "field": {
    ///      "description": "Field to check (e.g., 'page_filtered_text',
    /// 'page_title')",
    ///      "type": "string"
    ///    },
    ///    "operator": {
    ///      "description": "Comparison operator (e.g., 'contains_regex',
    /// 'equals', 'not_equals')",
    ///      "type": "string"
    ///    },
    ///    "value": {
    ///      "description": "Value to compare against",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct WatchBaseConditionsItem {
        ///Field to check (e.g., 'page_filtered_text', 'page_title')
        pub field: ::std::string::String,
        ///Comparison operator (e.g., 'contains_regex', 'equals', 'not_equals')
        pub operator: ::std::string::String,
        ///Value to compare against
        pub value: ::std::string::String,
    }

    ///Logic operator - ALL (match all conditions) or ANY (match any condition)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Logic operator - ALL (match all conditions) or ANY
    /// (match any condition)",
    ///  "default": "ALL",
    ///  "type": "string",
    ///  "enum": [
    ///    "ALL",
    ///    "ANY"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum WatchBaseConditionsMatchLogic {
        #[serde(rename = "ALL")]
        All,
        #[serde(rename = "ANY")]
        Any,
    }

    impl ::std::fmt::Display for WatchBaseConditionsMatchLogic {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::All => f.write_str("ALL"),
                Self::Any => f.write_str("ANY"),
            }
        }
    }

    impl ::std::str::FromStr for WatchBaseConditionsMatchLogic {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "ALL" => Ok(Self::All),
                "ANY" => Ok(Self::Any),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for WatchBaseConditionsMatchLogic {
        fn default() -> Self {
            WatchBaseConditionsMatchLogic::All
        }
    }

    ///`WatchBaseExtractLinesContainingItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseExtractLinesContainingItem(::std::string::String);
    impl ::std::ops::Deref for WatchBaseExtractLinesContainingItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseExtractLinesContainingItem> for ::std::string::String {
        fn from(value: WatchBaseExtractLinesContainingItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseExtractLinesContainingItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseExtractLinesContainingItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBaseExtractTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseExtractTextItem(::std::string::String);
    impl ::std::ops::Deref for WatchBaseExtractTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseExtractTextItem> for ::std::string::String {
        fn from(value: WatchBaseExtractTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseExtractTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseExtractTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Backend to use for fetching content. Common values:
    /// - `system` (default) - Use the system-wide default fetcher
    /// - `html_requests` - Fast requests-based fetcher
    /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
    /// - `extra_browser_*` - Custom browser configurations (if configured)
    /// - Plugin-provided fetchers (if installed)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Backend to use for fetching content. Common values:\n-
    /// `system` (default) - Use the system-wide default fetcher\n-
    /// `html_requests` - Fast requests-based fetcher\n- `html_webdriver` -
    /// Browser-based fetcher (Playwright/Puppeteer)\n- `extra_browser_*` -
    /// Custom browser configurations (if configured)\n- Plugin-provided
    /// fetchers (if installed)\n",
    ///  "default": "system",
    ///  "type": "string",
    ///  "pattern": "^(system|html_requests|html_webdriver|extra_browser_.+)$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseFetchBackend(::std::string::String);
    impl ::std::ops::Deref for WatchBaseFetchBackend {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseFetchBackend> for ::std::string::String {
        fn from(value: WatchBaseFetchBackend) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for WatchBaseFetchBackend {
        fn default() -> Self {
            WatchBaseFetchBackend("system".to_string())
        }
    }

    impl ::std::str::FromStr for WatchBaseFetchBackend {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new(
                        "^(system|html_requests|html_webdriver|extra_browser_.+)$",
                    )
                    .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^(system|html_requests|html_webdriver|extra_browser_.+)$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseFetchBackend {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBaseIgnoreTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseIgnoreTextItem(::std::string::String);
    impl ::std::ops::Deref for WatchBaseIgnoreTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseIgnoreTextItem> for ::std::string::String {
        fn from(value: WatchBaseIgnoreTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseIgnoreTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseIgnoreTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBaseIncludeFiltersItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseIncludeFiltersItem(::std::string::String);
    impl ::std::ops::Deref for WatchBaseIncludeFiltersItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseIncludeFiltersItem> for ::std::string::String {
        fn from(value: WatchBaseIncludeFiltersItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseIncludeFiltersItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseIncludeFiltersItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///HTTP method to use
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP method to use",
    ///  "type": "string",
    ///  "enum": [
    ///    "GET",
    ///    "POST",
    ///    "DELETE",
    ///    "PUT"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum WatchBaseMethod {
        #[serde(rename = "GET")]
        Get,
        #[serde(rename = "POST")]
        Post,
        #[serde(rename = "DELETE")]
        Delete,
        #[serde(rename = "PUT")]
        Put,
    }

    impl ::std::fmt::Display for WatchBaseMethod {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Get => f.write_str("GET"),
                Self::Post => f.write_str("POST"),
                Self::Delete => f.write_str("DELETE"),
                Self::Put => f.write_str("PUT"),
            }
        }
    }

    impl ::std::str::FromStr for WatchBaseMethod {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "GET" => Ok(Self::Get),
                "POST" => Ok(Self::Post),
                "DELETE" => Ok(Self::Delete),
                "PUT" => Ok(Self::Put),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseMethod {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseNotificationBody(::std::string::String);
    impl ::std::ops::Deref for WatchBaseNotificationBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseNotificationBody> for ::std::string::String {
        fn from(value: WatchBaseNotificationBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseNotificationBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseNotificationBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Format for notifications
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Format for notifications",
    ///  "type": "string",
    ///  "enum": [
    ///    "text",
    ///    "html",
    ///    "htmlcolor",
    ///    "markdown",
    ///    "System default"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum WatchBaseNotificationFormat {
        #[serde(rename = "text")]
        Text,
        #[serde(rename = "html")]
        Html,
        #[serde(rename = "htmlcolor")]
        Htmlcolor,
        #[serde(rename = "markdown")]
        Markdown,
        #[serde(rename = "System default")]
        SystemDefault,
    }

    impl ::std::fmt::Display for WatchBaseNotificationFormat {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Text => f.write_str("text"),
                Self::Html => f.write_str("html"),
                Self::Htmlcolor => f.write_str("htmlcolor"),
                Self::Markdown => f.write_str("markdown"),
                Self::SystemDefault => f.write_str("System default"),
            }
        }
    }

    impl ::std::str::FromStr for WatchBaseNotificationFormat {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "text" => Ok(Self::Text),
                "html" => Ok(Self::Html),
                "htmlcolor" => Ok(Self::Htmlcolor),
                "markdown" => Ok(Self::Markdown),
                "System default" => Ok(Self::SystemDefault),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseNotificationTitle(::std::string::String);
    impl ::std::ops::Deref for WatchBaseNotificationTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseNotificationTitle> for ::std::string::String {
        fn from(value: WatchBaseNotificationTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseNotificationTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseNotificationTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBaseNotificationUrlsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 1000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseNotificationUrlsItem(::std::string::String);
    impl ::std::ops::Deref for WatchBaseNotificationUrlsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseNotificationUrlsItem> for ::std::string::String {
        fn from(value: WatchBaseNotificationUrlsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseNotificationUrlsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 1000usize {
                return Err("longer than 1000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseNotificationUrlsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Optional processor mode to use for change detection. Defaults to
    /// `text_json_diff` if not specified.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Optional processor mode to use for change detection.
    /// Defaults to `text_json_diff` if not specified.",
    ///  "default": "text_json_diff",
    ///  "type": "string",
    ///  "enum": [
    ///    "restock_diff",
    ///    "text_json_diff"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum WatchBaseProcessor {
        #[serde(rename = "restock_diff")]
        RestockDiff,
        #[serde(rename = "text_json_diff")]
        TextJsonDiff,
    }

    impl ::std::fmt::Display for WatchBaseProcessor {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::RestockDiff => f.write_str("restock_diff"),
                Self::TextJsonDiff => f.write_str("text_json_diff"),
            }
        }
    }

    impl ::std::str::FromStr for WatchBaseProcessor {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "restock_diff" => Ok(Self::RestockDiff),
                "text_json_diff" => Ok(Self::TextJsonDiff),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseProcessor {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for WatchBaseProcessor {
        fn default() -> Self {
            WatchBaseProcessor::TextJsonDiff
        }
    }

    ///Proxy configuration
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Proxy configuration",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseProxy(::std::string::String);
    impl ::std::ops::Deref for WatchBaseProxy {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseProxy> for ::std::string::String {
        fn from(value: WatchBaseProxy) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseProxy {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseProxy {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseProxy {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBaseSubtractiveSelectorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseSubtractiveSelectorsItem(::std::string::String);
    impl ::std::ops::Deref for WatchBaseSubtractiveSelectorsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseSubtractiveSelectorsItem> for ::std::string::String {
        fn from(value: WatchBaseSubtractiveSelectorsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseSubtractiveSelectorsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseSubtractiveSelectorsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Tag UUID to associate with this web page change monitor (watch)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Tag UUID to associate with this web page change monitor
    /// (watch)",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseTag(::std::string::String);
    impl ::std::ops::Deref for WatchBaseTag {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseTag> for ::std::string::String {
        fn from(value: WatchBaseTag) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseTag {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseTag {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseTag {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBaseTextShouldNotBePresentItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseTextShouldNotBePresentItem(::std::string::String);
    impl ::std::ops::Deref for WatchBaseTextShouldNotBePresentItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseTextShouldNotBePresentItem> for ::std::string::String {
        fn from(value: WatchBaseTextShouldNotBePresentItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseTextShouldNotBePresentItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseTextShouldNotBePresentItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Time intervals between checks. All fields must be non-negative. At least
    /// one non-zero value required when not using default settings.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Time intervals between checks. All fields must be
    /// non-negative. At least one non-zero value required when not using
    /// default settings.",
    ///  "type": "object",
    ///  "properties": {
    ///    "days": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 365000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "hours": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 8760000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "minutes": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 525600000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "seconds": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 31536000000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "weeks": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 52000.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct WatchBaseTimeBetweenCheck {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub days: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub hours: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub minutes: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub seconds: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub weeks: ::std::option::Option<i64>,
    }

    impl ::std::default::Default for WatchBaseTimeBetweenCheck {
        fn default() -> Self {
            Self {
                days: Default::default(),
                hours: Default::default(),
                minutes: Default::default(),
                seconds: Default::default(),
                weeks: Default::default(),
            }
        }
    }

    ///Weekly schedule limiting when checks can run
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Weekly schedule limiting when checks can run",
    ///  "type": "object",
    ///  "properties": {
    ///    "enabled": {
    ///      "default": false,
    ///      "type": "boolean"
    ///    },
    ///    "friday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "monday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "saturday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "sunday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "thursday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "tuesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "wednesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct WatchBaseTimeScheduleLimit {
        #[serde(default)]
        pub enabled: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub friday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub monday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub saturday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub sunday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub thursday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tuesday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub wednesday: ::std::option::Option<DaySchedule>,
    }

    impl ::std::default::Default for WatchBaseTimeScheduleLimit {
        fn default() -> Self {
            Self {
                enabled: Default::default(),
                friday: Default::default(),
                monday: Default::default(),
                saturday: Default::default(),
                sunday: Default::default(),
                thursday: Default::default(),
                tuesday: Default::default(),
                wednesday: Default::default(),
            }
        }
    }

    ///Custom title for the web page change monitor (watch), not to be confused
    /// with page_title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom title for the web page change monitor (watch),
    /// not to be confused with page_title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseTitle(::std::string::String);
    impl ::std::ops::Deref for WatchBaseTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseTitle> for ::std::string::String {
        fn from(value: WatchBaseTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBaseTriggerTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseTriggerTextItem(::std::string::String);
    impl ::std::ops::Deref for WatchBaseTriggerTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseTriggerTextItem> for ::std::string::String {
        fn from(value: WatchBaseTriggerTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseTriggerTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseTriggerTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///JavaScript code to execute
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "JavaScript code to execute",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBaseWebdriverJsExecuteCode(::std::string::String);
    impl ::std::ops::Deref for WatchBaseWebdriverJsExecuteCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBaseWebdriverJsExecuteCode> for ::std::string::String {
        fn from(value: WatchBaseWebdriverJsExecuteCode) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBaseWebdriverJsExecuteCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBaseWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBaseWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBaseWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBaseWebdriverJsExecuteCode {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///HTTP request body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP request body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBody(::std::string::String);
    impl ::std::ops::Deref for WatchBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBody> for ::std::string::String {
        fn from(value: WatchBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBrowserStepsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "operation",
    ///    "optional_value",
    ///    "selector"
    ///  ],
    ///  "properties": {
    ///    "operation": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "optional_value": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    },
    ///    "selector": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 5000
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct WatchBrowserStepsItem {
        pub operation: ::std::option::Option<WatchBrowserStepsItemOperation>,
        pub optional_value: ::std::option::Option<WatchBrowserStepsItemOptionalValue>,
        pub selector: ::std::option::Option<WatchBrowserStepsItemSelector>,
    }

    ///`WatchBrowserStepsItemOperation`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBrowserStepsItemOperation(::std::string::String);
    impl ::std::ops::Deref for WatchBrowserStepsItemOperation {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBrowserStepsItemOperation> for ::std::string::String {
        fn from(value: WatchBrowserStepsItemOperation) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBrowserStepsItemOperation {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBrowserStepsItemOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBrowserStepsItemOperation {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBrowserStepsItemOptionalValue`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBrowserStepsItemOptionalValue(::std::string::String);
    impl ::std::ops::Deref for WatchBrowserStepsItemOptionalValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBrowserStepsItemOptionalValue> for ::std::string::String {
        fn from(value: WatchBrowserStepsItemOptionalValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBrowserStepsItemOptionalValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBrowserStepsItemOptionalValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBrowserStepsItemOptionalValue {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchBrowserStepsItemSelector`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchBrowserStepsItemSelector(::std::string::String);
    impl ::std::ops::Deref for WatchBrowserStepsItemSelector {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchBrowserStepsItemSelector> for ::std::string::String {
        fn from(value: WatchBrowserStepsItemSelector) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchBrowserStepsItemSelector {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchBrowserStepsItemSelector {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchBrowserStepsItemSelector {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchConditionsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "field",
    ///    "operator",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "field": {
    ///      "description": "Field to check (e.g., 'page_filtered_text',
    /// 'page_title')",
    ///      "type": "string"
    ///    },
    ///    "operator": {
    ///      "description": "Comparison operator (e.g., 'contains_regex',
    /// 'equals', 'not_equals')",
    ///      "type": "string"
    ///    },
    ///    "value": {
    ///      "description": "Value to compare against",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct WatchConditionsItem {
        ///Field to check (e.g., 'page_filtered_text', 'page_title')
        pub field: ::std::string::String,
        ///Comparison operator (e.g., 'contains_regex', 'equals', 'not_equals')
        pub operator: ::std::string::String,
        ///Value to compare against
        pub value: ::std::string::String,
    }

    ///Logic operator - ALL (match all conditions) or ANY (match any condition)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Logic operator - ALL (match all conditions) or ANY
    /// (match any condition)",
    ///  "default": "ALL",
    ///  "type": "string",
    ///  "enum": [
    ///    "ALL",
    ///    "ANY"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum WatchConditionsMatchLogic {
        #[serde(rename = "ALL")]
        All,
        #[serde(rename = "ANY")]
        Any,
    }

    impl ::std::fmt::Display for WatchConditionsMatchLogic {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::All => f.write_str("ALL"),
                Self::Any => f.write_str("ANY"),
            }
        }
    }

    impl ::std::str::FromStr for WatchConditionsMatchLogic {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "ALL" => Ok(Self::All),
                "ANY" => Ok(Self::Any),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchConditionsMatchLogic {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for WatchConditionsMatchLogic {
        fn default() -> Self {
            WatchConditionsMatchLogic::All
        }
    }

    ///`WatchExtractLinesContainingItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchExtractLinesContainingItem(::std::string::String);
    impl ::std::ops::Deref for WatchExtractLinesContainingItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchExtractLinesContainingItem> for ::std::string::String {
        fn from(value: WatchExtractLinesContainingItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchExtractLinesContainingItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchExtractLinesContainingItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchExtractLinesContainingItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchExtractTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchExtractTextItem(::std::string::String);
    impl ::std::ops::Deref for WatchExtractTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchExtractTextItem> for ::std::string::String {
        fn from(value: WatchExtractTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchExtractTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchExtractTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchExtractTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Backend to use for fetching content. Common values:
    /// - `system` (default) - Use the system-wide default fetcher
    /// - `html_requests` - Fast requests-based fetcher
    /// - `html_webdriver` - Browser-based fetcher (Playwright/Puppeteer)
    /// - `extra_browser_*` - Custom browser configurations (if configured)
    /// - Plugin-provided fetchers (if installed)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Backend to use for fetching content. Common values:\n-
    /// `system` (default) - Use the system-wide default fetcher\n-
    /// `html_requests` - Fast requests-based fetcher\n- `html_webdriver` -
    /// Browser-based fetcher (Playwright/Puppeteer)\n- `extra_browser_*` -
    /// Custom browser configurations (if configured)\n- Plugin-provided
    /// fetchers (if installed)\n",
    ///  "default": "system",
    ///  "type": "string",
    ///  "pattern": "^(system|html_requests|html_webdriver|extra_browser_.+)$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchFetchBackend(::std::string::String);
    impl ::std::ops::Deref for WatchFetchBackend {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchFetchBackend> for ::std::string::String {
        fn from(value: WatchFetchBackend) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for WatchFetchBackend {
        fn default() -> Self {
            WatchFetchBackend("system".to_string())
        }
    }

    impl ::std::str::FromStr for WatchFetchBackend {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new(
                        "^(system|html_requests|html_webdriver|extra_browser_.+)$",
                    )
                    .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^(system|html_requests|html_webdriver|extra_browser_.+)$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchFetchBackend {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchFetchBackend {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Dictionary of timestamps and snapshot paths
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Dictionary of timestamps and snapshot paths",
    ///  "type": "object",
    ///  "additionalProperties": {
    ///    "description": "Path to snapshot file",
    ///    "type": "string"
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct WatchHistory(
        pub ::std::collections::HashMap<::std::string::String, ::std::string::String>,
    );
    impl ::std::ops::Deref for WatchHistory {
        type Target = ::std::collections::HashMap<::std::string::String, ::std::string::String>;
        fn deref(
            &self,
        ) -> &::std::collections::HashMap<::std::string::String, ::std::string::String> {
            &self.0
        }
    }

    impl ::std::convert::From<WatchHistory>
        for ::std::collections::HashMap<::std::string::String, ::std::string::String>
    {
        fn from(value: WatchHistory) -> Self {
            value.0
        }
    }

    impl
        ::std::convert::From<
            ::std::collections::HashMap<::std::string::String, ::std::string::String>,
        > for WatchHistory
    {
        fn from(
            value: ::std::collections::HashMap<::std::string::String, ::std::string::String>,
        ) -> Self {
            Self(value)
        }
    }

    ///`WatchIgnoreTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchIgnoreTextItem(::std::string::String);
    impl ::std::ops::Deref for WatchIgnoreTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchIgnoreTextItem> for ::std::string::String {
        fn from(value: WatchIgnoreTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchIgnoreTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchIgnoreTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchIgnoreTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchIncludeFiltersItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchIncludeFiltersItem(::std::string::String);
    impl ::std::ops::Deref for WatchIncludeFiltersItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchIncludeFiltersItem> for ::std::string::String {
        fn from(value: WatchIncludeFiltersItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchIncludeFiltersItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchIncludeFiltersItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchIncludeFiltersItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///HTTP method to use
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "HTTP method to use",
    ///  "type": "string",
    ///  "enum": [
    ///    "GET",
    ///    "POST",
    ///    "DELETE",
    ///    "PUT"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum WatchMethod {
        #[serde(rename = "GET")]
        Get,
        #[serde(rename = "POST")]
        Post,
        #[serde(rename = "DELETE")]
        Delete,
        #[serde(rename = "PUT")]
        Put,
    }

    impl ::std::fmt::Display for WatchMethod {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Get => f.write_str("GET"),
                Self::Post => f.write_str("POST"),
                Self::Delete => f.write_str("DELETE"),
                Self::Put => f.write_str("PUT"),
            }
        }
    }

    impl ::std::str::FromStr for WatchMethod {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "GET" => Ok(Self::Get),
                "POST" => Ok(Self::Post),
                "DELETE" => Ok(Self::Delete),
                "PUT" => Ok(Self::Put),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchMethod {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification body
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification body",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchNotificationBody(::std::string::String);
    impl ::std::ops::Deref for WatchNotificationBody {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchNotificationBody> for ::std::string::String {
        fn from(value: WatchNotificationBody) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchNotificationBody {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchNotificationBody {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchNotificationBody {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Format for notifications
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Format for notifications",
    ///  "type": "string",
    ///  "enum": [
    ///    "text",
    ///    "html",
    ///    "htmlcolor",
    ///    "markdown",
    ///    "System default"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum WatchNotificationFormat {
        #[serde(rename = "text")]
        Text,
        #[serde(rename = "html")]
        Html,
        #[serde(rename = "htmlcolor")]
        Htmlcolor,
        #[serde(rename = "markdown")]
        Markdown,
        #[serde(rename = "System default")]
        SystemDefault,
    }

    impl ::std::fmt::Display for WatchNotificationFormat {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Text => f.write_str("text"),
                Self::Html => f.write_str("html"),
                Self::Htmlcolor => f.write_str("htmlcolor"),
                Self::Markdown => f.write_str("markdown"),
                Self::SystemDefault => f.write_str("System default"),
            }
        }
    }

    impl ::std::str::FromStr for WatchNotificationFormat {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "text" => Ok(Self::Text),
                "html" => Ok(Self::Html),
                "htmlcolor" => Ok(Self::Htmlcolor),
                "markdown" => Ok(Self::Markdown),
                "System default" => Ok(Self::SystemDefault),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchNotificationFormat {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Custom notification title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom notification title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchNotificationTitle(::std::string::String);
    impl ::std::ops::Deref for WatchNotificationTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchNotificationTitle> for ::std::string::String {
        fn from(value: WatchNotificationTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchNotificationTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchNotificationTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchNotificationTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchNotificationUrlsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 1000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchNotificationUrlsItem(::std::string::String);
    impl ::std::ops::Deref for WatchNotificationUrlsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchNotificationUrlsItem> for ::std::string::String {
        fn from(value: WatchNotificationUrlsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchNotificationUrlsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 1000usize {
                return Err("longer than 1000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchNotificationUrlsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchNotificationUrlsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Optional processor mode to use for change detection. Defaults to
    /// `text_json_diff` if not specified.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Optional processor mode to use for change detection.
    /// Defaults to `text_json_diff` if not specified.",
    ///  "default": "text_json_diff",
    ///  "type": "string",
    ///  "enum": [
    ///    "restock_diff",
    ///    "text_json_diff"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum WatchProcessor {
        #[serde(rename = "restock_diff")]
        RestockDiff,
        #[serde(rename = "text_json_diff")]
        TextJsonDiff,
    }

    impl ::std::fmt::Display for WatchProcessor {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::RestockDiff => f.write_str("restock_diff"),
                Self::TextJsonDiff => f.write_str("text_json_diff"),
            }
        }
    }

    impl ::std::str::FromStr for WatchProcessor {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "restock_diff" => Ok(Self::RestockDiff),
                "text_json_diff" => Ok(Self::TextJsonDiff),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchProcessor {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchProcessor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for WatchProcessor {
        fn default() -> Self {
            WatchProcessor::TextJsonDiff
        }
    }

    ///Resolved restock/price processor config for this watch.
    ///If a tag with `overrides_watch: true` is assigned to this watch, the
    /// tag's config is returned here instead of the watch's own config. Use
    /// `processor_config_restock_diff_source` to determine where the config
    /// originated.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Resolved restock/price processor config for this
    /// watch.\nIf a tag with `overrides_watch: true` is assigned to this watch,
    /// the tag's config is\nreturned here instead of the watch's own config.
    /// Use `processor_config_restock_diff_source`\nto determine where the
    /// config originated.\n",
    ///  "readOnly": true,
    ///  "type": "object",
    ///  "properties": {
    ///    "follow_price_changes": {
    ///      "type": "boolean"
    ///    },
    ///    "in_stock_processing": {
    ///      "type": "string",
    ///      "enum": [
    ///        "in_stock_only",
    ///        "all_changes",
    ///        "off"
    ///      ]
    ///    },
    ///    "price_change_max": {
    ///      "type": [
    ///        "number",
    ///        "null"
    ///      ]
    ///    },
    ///    "price_change_min": {
    ///      "type": [
    ///        "number",
    ///        "null"
    ///      ]
    ///    },
    ///    "price_change_threshold_percent": {
    ///      "type": [
    ///        "number",
    ///        "null"
    ///      ],
    ///      "maximum": 100.0,
    ///      "minimum": 0.0
    ///    }
    ///  },
    ///  "x-computed": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct WatchProcessorConfigRestockDiff {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub follow_price_changes: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub in_stock_processing:
            ::std::option::Option<WatchProcessorConfigRestockDiffInStockProcessing>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_change_max: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_change_min: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_change_threshold_percent: ::std::option::Option<f64>,
    }

    impl ::std::default::Default for WatchProcessorConfigRestockDiff {
        fn default() -> Self {
            Self {
                follow_price_changes: Default::default(),
                in_stock_processing: Default::default(),
                price_change_max: Default::default(),
                price_change_min: Default::default(),
                price_change_threshold_percent: Default::default(),
            }
        }
    }

    ///`WatchProcessorConfigRestockDiffInStockProcessing`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "in_stock_only",
    ///    "all_changes",
    ///    "off"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum WatchProcessorConfigRestockDiffInStockProcessing {
        #[serde(rename = "in_stock_only")]
        InStockOnly,
        #[serde(rename = "all_changes")]
        AllChanges,
        #[serde(rename = "off")]
        Off,
    }

    impl ::std::fmt::Display for WatchProcessorConfigRestockDiffInStockProcessing {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::InStockOnly => f.write_str("in_stock_only"),
                Self::AllChanges => f.write_str("all_changes"),
                Self::Off => f.write_str("off"),
            }
        }
    }

    impl ::std::str::FromStr for WatchProcessorConfigRestockDiffInStockProcessing {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "in_stock_only" => Ok(Self::InStockOnly),
                "all_changes" => Ok(Self::AllChanges),
                "off" => Ok(Self::Off),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchProcessorConfigRestockDiffInStockProcessing {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for WatchProcessorConfigRestockDiffInStockProcessing
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for WatchProcessorConfigRestockDiffInStockProcessing
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Proxy configuration
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Proxy configuration",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchProxy(::std::string::String);
    impl ::std::ops::Deref for WatchProxy {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchProxy> for ::std::string::String {
        fn from(value: WatchProxy) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchProxy {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchProxy {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchProxy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchProxy {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchSubtractiveSelectorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchSubtractiveSelectorsItem(::std::string::String);
    impl ::std::ops::Deref for WatchSubtractiveSelectorsItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchSubtractiveSelectorsItem> for ::std::string::String {
        fn from(value: WatchSubtractiveSelectorsItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchSubtractiveSelectorsItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchSubtractiveSelectorsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchSubtractiveSelectorsItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Tag UUID to associate with this web page change monitor (watch)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Tag UUID to associate with this web page change monitor
    /// (watch)",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchTag(::std::string::String);
    impl ::std::ops::Deref for WatchTag {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchTag> for ::std::string::String {
        fn from(value: WatchTag) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchTag {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchTag {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchTag {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchTextShouldNotBePresentItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchTextShouldNotBePresentItem(::std::string::String);
    impl ::std::ops::Deref for WatchTextShouldNotBePresentItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchTextShouldNotBePresentItem> for ::std::string::String {
        fn from(value: WatchTextShouldNotBePresentItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchTextShouldNotBePresentItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchTextShouldNotBePresentItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchTextShouldNotBePresentItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///Time intervals between checks. All fields must be non-negative. At least
    /// one non-zero value required when not using default settings.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Time intervals between checks. All fields must be
    /// non-negative. At least one non-zero value required when not using
    /// default settings.",
    ///  "type": "object",
    ///  "properties": {
    ///    "days": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 365000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "hours": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 8760000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "minutes": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 525600000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "seconds": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 31536000000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "weeks": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "maximum": 52000.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct WatchTimeBetweenCheck {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub days: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub hours: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub minutes: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub seconds: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub weeks: ::std::option::Option<i64>,
    }

    impl ::std::default::Default for WatchTimeBetweenCheck {
        fn default() -> Self {
            Self {
                days: Default::default(),
                hours: Default::default(),
                minutes: Default::default(),
                seconds: Default::default(),
                weeks: Default::default(),
            }
        }
    }

    ///Weekly schedule limiting when checks can run
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Weekly schedule limiting when checks can run",
    ///  "type": "object",
    ///  "properties": {
    ///    "enabled": {
    ///      "default": false,
    ///      "type": "boolean"
    ///    },
    ///    "friday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "monday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "saturday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "sunday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "thursday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "tuesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    },
    ///    "wednesday": {
    ///      "$ref": "#/components/schemas/DaySchedule"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct WatchTimeScheduleLimit {
        #[serde(default)]
        pub enabled: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub friday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub monday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub saturday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub sunday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub thursday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tuesday: ::std::option::Option<DaySchedule>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub wednesday: ::std::option::Option<DaySchedule>,
    }

    impl ::std::default::Default for WatchTimeScheduleLimit {
        fn default() -> Self {
            Self {
                enabled: Default::default(),
                friday: Default::default(),
                monday: Default::default(),
                saturday: Default::default(),
                sunday: Default::default(),
                thursday: Default::default(),
                tuesday: Default::default(),
                wednesday: Default::default(),
            }
        }
    }

    ///Custom title for the web page change monitor (watch), not to be confused
    /// with page_title
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom title for the web page change monitor (watch),
    /// not to be confused with page_title",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchTitle(::std::string::String);
    impl ::std::ops::Deref for WatchTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchTitle> for ::std::string::String {
        fn from(value: WatchTitle) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchTitle {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchTitle {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///`WatchTriggerTextItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchTriggerTextItem(::std::string::String);
    impl ::std::ops::Deref for WatchTriggerTextItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchTriggerTextItem> for ::std::string::String {
        fn from(value: WatchTriggerTextItem) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchTriggerTextItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchTriggerTextItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchTriggerTextItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    ///JavaScript code to execute
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "JavaScript code to execute",
    ///  "type": "string",
    ///  "maxLength": 5000
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct WatchWebdriverJsExecuteCode(::std::string::String);
    impl ::std::ops::Deref for WatchWebdriverJsExecuteCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<WatchWebdriverJsExecuteCode> for ::std::string::String {
        fn from(value: WatchWebdriverJsExecuteCode) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for WatchWebdriverJsExecuteCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for WatchWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for WatchWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for WatchWebdriverJsExecuteCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for WatchWebdriverJsExecuteCode {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }

    /// Generation of default values for serde.
    pub mod defaults {
        pub(super) fn default_bool<const V: bool>() -> bool {
            V
        }

        pub(super) fn create_tag_conditions_match_logic() -> super::CreateTagConditionsMatchLogic {
            super::CreateTagConditionsMatchLogic::All
        }

        pub(super) fn create_tag_fetch_backend() -> super::CreateTagFetchBackend {
            super::CreateTagFetchBackend("system".to_string())
        }

        pub(super) fn create_tag_processor() -> super::CreateTagProcessor {
            super::CreateTagProcessor::TextJsonDiff
        }

        pub(super) fn create_watch_conditions_match_logic() -> super::CreateWatchConditionsMatchLogic
        {
            super::CreateWatchConditionsMatchLogic::All
        }

        pub(super) fn create_watch_fetch_backend() -> super::CreateWatchFetchBackend {
            super::CreateWatchFetchBackend("system".to_string())
        }

        pub(super) fn create_watch_processor() -> super::CreateWatchProcessor {
            super::CreateWatchProcessor::TextJsonDiff
        }

        pub(super) fn day_schedule_start_time() -> super::DayScheduleStartTime {
            super::DayScheduleStartTime("00:00".to_string())
        }

        pub(super) fn day_schedule_duration_hours() -> super::DayScheduleDurationHours {
            super::DayScheduleDurationHours("24".to_string())
        }

        pub(super) fn day_schedule_duration_minutes() -> super::DayScheduleDurationMinutes {
            super::DayScheduleDurationMinutes("00".to_string())
        }

        pub(super) fn tag_conditions_match_logic() -> super::TagConditionsMatchLogic {
            super::TagConditionsMatchLogic::All
        }

        pub(super) fn tag_fetch_backend() -> super::TagFetchBackend {
            super::TagFetchBackend("system".to_string())
        }

        pub(super) fn tag_processor() -> super::TagProcessor {
            super::TagProcessor::TextJsonDiff
        }

        pub(super) fn update_watch_conditions_match_logic() -> super::UpdateWatchConditionsMatchLogic
        {
            super::UpdateWatchConditionsMatchLogic::All
        }

        pub(super) fn update_watch_fetch_backend() -> super::UpdateWatchFetchBackend {
            super::UpdateWatchFetchBackend("system".to_string())
        }

        pub(super) fn update_watch_processor() -> super::UpdateWatchProcessor {
            super::UpdateWatchProcessor::TextJsonDiff
        }

        pub(super) fn watch_conditions_match_logic() -> super::WatchConditionsMatchLogic {
            super::WatchConditionsMatchLogic::All
        }

        pub(super) fn watch_fetch_backend() -> super::WatchFetchBackend {
            super::WatchFetchBackend("system".to_string())
        }

        pub(super) fn watch_processor() -> super::WatchProcessor {
            super::WatchProcessor::TextJsonDiff
        }

        pub(super) fn watch_base_conditions_match_logic() -> super::WatchBaseConditionsMatchLogic {
            super::WatchBaseConditionsMatchLogic::All
        }

        pub(super) fn watch_base_fetch_backend() -> super::WatchBaseFetchBackend {
            super::WatchBaseFetchBackend("system".to_string())
        }

        pub(super) fn watch_base_processor() -> super::WatchBaseProcessor {
            super::WatchBaseProcessor::TextJsonDiff
        }
    }
}

#[derive(Clone, Debug)]
///Client for ChangeDetection.io API
///
///# ChangeDetection.io Web page monitoring and notifications API
///
///REST API for managing Page watches, Group tags, and Notifications.
///
///changedetection.io can be driven by its built in simple API, in the examples
/// below you will also find `curl` command line and `python` examples to help
/// you get started faster.
///
///## Where to find my API key?
///
///The API key can be easily found under the **SETTINGS** then **API** tab of
/// changedetection.io dashboard. Simply click the API key to automatically copy
/// it to your clipboard.
///
///![Where to find the API key](./where-to-get-api-key.jpeg)
///
///## Connection URL
///
///The API can be found at `/api/v1/`, so for example if you run changedetection.io locally on port 5000, then URL would be `http://localhost:5000/api/v1/watch/cc0cfffa-f449-477b-83ea-0caafd1dc091/history`.
///
///If you are using the hosted/subscription version of changedetection.io, then
/// the URL is based on your login URL, for example: `https://<your login url>/api/v1/watch/cc0cfffa-f449-477b-83ea-0caafd1dc091/history`
///
///## Authentication
///
///Almost all API requests require some authentication, this is provided as an
/// **API Key** in the header of the HTTP request.
///
///For example: `x-api-key: YOUR_API_KEY`
///
///
///Version: 0.1.7
pub struct Client {
    pub(crate) baseurl: String,
    pub(crate) client: reqwest::Client,
}

impl Client {
    /// Create a new client.
    ///
    /// `baseurl` is the base URL provided to the internal
    /// `reqwest::Client`, and should include a scheme and hostname,
    /// as well as port and a path stem if applicable.
    pub fn new(baseurl: &str) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let client = {
            let dur = ::std::time::Duration::from_secs(15u64);
            reqwest::ClientBuilder::new()
                .connect_timeout(dur)
                .timeout(dur)
        };
        #[cfg(target_arch = "wasm32")]
        let client = reqwest::ClientBuilder::new();
        Self::new_with_client(baseurl, client.build().unwrap())
    }

    /// Construct a new client with an existing `reqwest::Client`,
    /// allowing more control over its configuration.
    ///
    /// `baseurl` is the base URL provided to the internal
    /// `reqwest::Client`, and should include a scheme and hostname,
    /// as well as port and a path stem if applicable.
    pub fn new_with_client(baseurl: &str, client: reqwest::Client) -> Self {
        Self {
            baseurl: baseurl.to_string(),
            client,
        }
    }
}

impl ClientInfo<()> for Client {
    fn api_version() -> &'static str {
        "0.1.7"
    }

    fn baseurl(&self) -> &str {
        self.baseurl.as_str()
    }

    fn client(&self) -> &reqwest::Client {
        &self.client
    }

    fn inner(&self) -> &() {
        &()
    }
}

impl ClientHooks<()> for &Client {}
#[allow(clippy::all)]
impl Client {
    ///List all watches
    ///
    ///Return concise list of available web page change monitors (watches) and
    /// basic info
    ///
    ///Sends a `GET` request to `/watch`
    ///
    ///Arguments:
    /// - `recheck_all`: Set to 1 to force recheck of all watches
    /// - `tag`: Tag name to filter results
    pub async fn list_watches<'a>(
        &'a self,
        recheck_all: Option<types::ListWatchesRecheckAll>,
        tag: Option<&'a str>,
    ) -> Result<
        ResponseValue<::std::collections::HashMap<::std::string::String, types::Watch>>,
        Error<()>,
    > {
        let url = format!("{}/watch", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new(
                "recheck_all",
                &recheck_all,
            ))
            .query(&progenitor_client::QueryParam::new("tag", &tag))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "list_watches",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Create a new watch
    ///
    ///Create a single web page change monitor (watch). Requires at least `url`
    /// to be set.
    ///
    ///Every watch can be configured with:
    /// - **Processor mode**: `processor` field (`restock_diff` or
    ///   `text_json_diff` - default)
    /// - **Notification settings**: `notification_urls` (array),
    ///   `notification_title`, `notification_body`, `notification_format`,
    ///   `notification_muted`
    /// - **Tags/Groups**: `tag` (UUID string) or `tags` (array of UUIDs)
    /// - **Check settings**: `time_between_check`, `paused`, `method`,
    ///   `fetch_backend`
    /// - **Advanced options**: `headers`, `body`, `proxy`, `browser_steps`, and
    ///   more
    ///
    ///
    ///Sends a `POST` request to `/watch`
    pub async fn create_watch<'a>(
        &'a self,
        body: &'a types::CreateWatch,
    ) -> Result<ResponseValue<ByteStream>, Error<ByteStream>> {
        let url = format!("{}/watch", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "create_watch",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::stream(response)),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get single watch
    ///
    ///Retrieve web page change monitor (watch) information and set
    /// muted/paused status. Returns the FULL Watch JSON.
    ///
    ///Sends a `GET` request to `/watch/{uuid}`
    ///
    ///Arguments:
    /// - `uuid`: Web page change monitor (watch) unique ID
    /// - `muted`: Set mute state
    /// - `paused`: Set pause state
    /// - `recheck`: Recheck this web page change monitor (watch)
    pub async fn get_watch<'a>(
        &'a self,
        uuid: &'a ::uuid::Uuid,
        muted: Option<types::GetWatchMuted>,
        paused: Option<types::GetWatchPaused>,
        recheck: Option<types::GetWatchRecheck>,
    ) -> Result<ResponseValue<types::Watch>, Error<types::Error>> {
        let url = format!("{}/watch/{}", self.baseurl, encode_path(&uuid.to_string()),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new("muted", &muted))
            .query(&progenitor_client::QueryParam::new("paused", &paused))
            .query(&progenitor_client::QueryParam::new("recheck", &recheck))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_watch",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            404u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update watch
    ///
    ///Update an existing web page change monitor (watch) using JSON. Accepts
    /// the same structure as returned in [get single watch
    /// information](#operation/getWatch).
    ///
    ///Sends a `PUT` request to `/watch/{uuid}`
    ///
    ///Arguments:
    /// - `uuid`: Web page change monitor (watch) unique ID
    /// - `body`
    pub async fn update_watch<'a>(
        &'a self,
        uuid: &'a ::uuid::Uuid,
        body: &'a types::UpdateWatch,
    ) -> Result<ResponseValue<ByteStream>, Error<()>> {
        let url = format!("{}/watch/{}", self.baseurl, encode_path(&uuid.to_string()),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .put(url)
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "update_watch",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::stream(response)),
            500u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete watch
    ///
    ///Delete a web page change monitor (watch) and all related history
    ///
    ///Sends a `DELETE` request to `/watch/{uuid}`
    ///
    ///Arguments:
    /// - `uuid`: Web page change monitor (watch) unique ID
    pub async fn delete_watch<'a>(
        &'a self,
        uuid: &'a ::uuid::Uuid,
    ) -> Result<ResponseValue<ByteStream>, Error<()>> {
        let url = format!("{}/watch/{}", self.baseurl, encode_path(&uuid.to_string()),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self.client.delete(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "delete_watch",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::stream(response)),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get watch history
    ///
    ///Get a list of all historical snapshots available for a web page change
    /// monitor (watch), use the key `timestamp` as the query argument for
    /// fetching a single watch history snapshot.
    ///
    ///
    ///Sends a `GET` request to `/watch/{uuid}/history`
    ///
    ///Arguments:
    /// - `uuid`: Web page change monitor (watch) unique ID
    pub async fn get_watch_history<'a>(
        &'a self,
        uuid: &'a ::uuid::Uuid,
    ) -> Result<ResponseValue<types::WatchHistory>, Error<()>> {
        let url = format!(
            "{}/watch/{}/history",
            self.baseurl,
            encode_path(&uuid.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_watch_history",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            404u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get single snapshot
    ///
    ///Get single snapshot from web page change monitor (watch). Use 'latest'
    /// for the most recent snapshot. Use the Watch History API to get a
    /// list of timestamps to pass.
    ///
    ///
    ///Sends a `GET` request to `/watch/{uuid}/history/{timestamp}`
    ///
    ///Arguments:
    /// - `uuid`: Web page change monitor (watch) unique ID
    /// - `timestamp`: Snapshot timestamp or 'latest'
    /// - `html`: Set to 1 to return the last HTML
    pub async fn get_watch_snapshot<'a>(
        &'a self,
        uuid: &'a ::uuid::Uuid,
        timestamp: &'a types::GetWatchSnapshotTimestamp,
        html: Option<types::GetWatchSnapshotHtml>,
    ) -> Result<ResponseValue<ByteStream>, Error<()>> {
        let url = format!(
            "{}/watch/{}/history/{}",
            self.baseurl,
            encode_path(&uuid.to_string()),
            encode_path(&timestamp.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .query(&progenitor_client::QueryParam::new("html", &html))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_watch_snapshot",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::stream(response)),
            404u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get the difference between two snapshots
    ///
    ///Generate a difference (comparison) between two historical snapshots of a
    /// web page change monitor (watch).
    ///
    ///This endpoint compares content between two points in time and returns
    /// the differences in your chosen format. Perfect for reviewing what
    /// changed between specific versions or comparing recent changes.
    ///
    ///**Timestamp Keywords:**
    /// - Use `'latest'` for the most recent snapshot (to_timestamp)
    /// - Use `'previous'` for the second-most-recent snapshot (from_timestamp)
    /// - Or use specific Unix timestamps from the watch history
    ///
    ///**Format Options:**
    /// - `text` (default): Plain text with (removed) and (added) prefixes
    /// - `html`: HTML format with (removed) and (added) text
    /// - `htmlcolor`: Rich HTML with colored highlights (green for additions,
    ///   red for deletions)
    ///
    ///**Word-Level Diffing:**
    /// - Enable word-level granularity with `word_diff=true` for detailed
    ///   inline comparisons
    /// - Disable with `word_diff=false` for line-level comparisons only
    ///   (default false/off, line-level mode by default)
    ///
    ///**Raw Diff Output:**
    /// - Use `no_markup=true` to get raw diff content without any formatting
    ///   applied
    /// - Returns content with placeholders for opening/closing tags of changes
    /// - Allows you to implement your own custom colorisation or formatting
    /// - Skips all HTML color application and service tweaks (added text, html
    ///   color tags, etc)
    ///
    ///
    ///Sends a `GET` request to
    /// `/watch/{uuid}/difference/{from_timestamp}/{to_timestamp}`
    ///
    ///Arguments:
    /// - `uuid`: Web page change monitor (watch) unique ID
    /// - `from_timestamp`: Starting snapshot timestamp, 'previous' for
    ///   second-most-recent, or specific Unix timestamp
    /// - `to_timestamp`: Ending snapshot timestamp, 'latest' for most recent,
    ///   or specific Unix timestamp
    /// - `added`: Include added/new content in the diff output.
    ///When disabled, content that was added will not appear in the diff.
    ///Accepts: true, false, 1, 0, yes, no, on, off
    ///
    /// - `changes_only`: When enabled, only show lines/content that changed (no
    ///   surrounding context).
    ///When disabled, include unchanged lines for context around changes.
    ///Accepts: true, false, 1, 0, yes, no, on, off
    ///
    /// - `format`: Output format for the diff:
    /// - `text` (default): Plain text with (removed) and (added) prefixes
    /// - `html`: Basic HTML format
    /// - `htmlcolor`: Rich HTML with colored backgrounds (red for deletions,
    ///   green for additions)
    /// - `markdown`: Markdown format with HTML rendering
    ///
    /// - `ignore_whitespace`: When enabled, ignore whitespace-only changes
    ///   (spaces, tabs, newlines).
    ///Useful for focusing on content changes and ignoring formatting
    /// differences. Accepts: true, false, 1, 0, yes, no, on, off
    ///
    /// - `no_markup`: When set to true, returns the raw diff content without
    ///   any markup formatting.
    ///The content will include placeholders for opening/closing tags of the
    /// changes, allowing you to implement your own custom colorisation or
    /// formatting. This skips all HTML color application and service
    /// tweaks. Accepts: true, false, 1, 0, yes, no, on, off
    ///
    /// - `removed`: Include removed/deleted content in the diff output.
    ///When disabled, content that was deleted will not appear in the diff.
    ///Accepts: true, false, 1, 0, yes, no, on, off
    ///
    /// - `replaced`: Include replaced/modified content in the diff output.
    ///When disabled, content that was modified (changed from one value to
    /// another) will not appear in the diff. Accepts: true, false, 1, 0,
    /// yes, no, on, off
    ///
    /// - `type_`: Diff granularity type:
    /// - `diffLines` (default): Line-level comparison, showing which lines
    ///   changed
    /// - `diffWords`: Word-level comparison, showing which words changed within
    ///   lines
    ///
    ///This parameter is an alternative to `word_diff` for better alignment
    /// with the UI. If both are specified, `type=diffWords` will enable
    /// word-level diffing.
    ///
    /// - `word_diff`: Enable word-level diffing for more granular comparisons.
    ///When enabled, changes are highlighted at the word level rather than line
    /// level. Default is false (line-level mode).
    ///Accepts: true, false, 1, 0, yes, no, on, off
    pub async fn get_watch_history_diff<'a>(
        &'a self,
        uuid: &'a ::uuid::Uuid,
        from_timestamp: &'a types::GetWatchHistoryDiffFromTimestamp,
        to_timestamp: &'a types::GetWatchHistoryDiffToTimestamp,
        added: Option<types::GetWatchHistoryDiffAdded>,
        changes_only: Option<types::GetWatchHistoryDiffChangesOnly>,
        format: Option<types::GetWatchHistoryDiffFormat>,
        ignore_whitespace: Option<types::GetWatchHistoryDiffIgnoreWhitespace>,
        no_markup: Option<types::GetWatchHistoryDiffNoMarkup>,
        removed: Option<types::GetWatchHistoryDiffRemoved>,
        replaced: Option<types::GetWatchHistoryDiffReplaced>,
        type_: Option<types::GetWatchHistoryDiffType>,
        word_diff: Option<types::GetWatchHistoryDiffWordDiff>,
    ) -> Result<ResponseValue<ByteStream>, Error<()>> {
        let url = format!(
            "{}/watch/{}/difference/{}/{}",
            self.baseurl,
            encode_path(&uuid.to_string()),
            encode_path(&from_timestamp.to_string()),
            encode_path(&to_timestamp.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .query(&progenitor_client::QueryParam::new("added", &added))
            .query(&progenitor_client::QueryParam::new(
                "changesOnly",
                &changes_only,
            ))
            .query(&progenitor_client::QueryParam::new("format", &format))
            .query(&progenitor_client::QueryParam::new(
                "ignoreWhitespace",
                &ignore_whitespace,
            ))
            .query(&progenitor_client::QueryParam::new("no_markup", &no_markup))
            .query(&progenitor_client::QueryParam::new("removed", &removed))
            .query(&progenitor_client::QueryParam::new("replaced", &replaced))
            .query(&progenitor_client::QueryParam::new("type", &type_))
            .query(&progenitor_client::QueryParam::new("word_diff", &word_diff))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_watch_history_diff",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::stream(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get watch favicon
    ///
    ///Get the favicon for a web page change monitor (watch) as displayed in
    /// the watch overview list.
    ///
    ///Sends a `GET` request to `/watch/{uuid}/favicon`
    ///
    ///Arguments:
    /// - `uuid`: Web page change monitor (watch) unique ID
    pub async fn get_watch_favicon<'a>(
        &'a self,
        uuid: &'a ::uuid::Uuid,
    ) -> Result<ResponseValue<ByteStream>, Error<()>> {
        let url = format!(
            "{}/watch/{}/favicon",
            self.baseurl,
            encode_path(&uuid.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self.client.get(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "get_watch_favicon",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::stream(response)),
            404u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///List all tags
    ///
    ///Return list of available tags/groups
    ///
    ///Sends a `GET` request to `/tags`
    pub async fn list_tags<'a>(
        &'a self,
    ) -> Result<
        ResponseValue<::std::collections::HashMap<::std::string::String, types::Tag>>,
        Error<()>,
    > {
        let url = format!("{}/tags", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "list_tags",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Create tag
    ///
    ///Create a single tag/group
    ///
    ///Sends a `POST` request to `/tag`
    pub async fn create_tag<'a>(
        &'a self,
        body: &'a types::CreateTag,
    ) -> Result<ResponseValue<types::CreateTagResponse>, Error<()>> {
        let url = format!("{}/tag", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "create_tag",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            201u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get single tag
    ///
    ///Retrieve tag information, set notification_muted status, recheck all web
    /// page change monitors (watches) in tag.
    ///
    ///Sends a `GET` request to `/tag/{uuid}`
    ///
    ///Arguments:
    /// - `uuid`: Tag unique ID
    /// - `muted`: Set mute state
    /// - `recheck`: Queue all web page change monitors (watches) with this tag
    ///   for recheck
    pub async fn get_tag<'a>(
        &'a self,
        uuid: &'a ::uuid::Uuid,
        muted: Option<types::GetTagMuted>,
        recheck: Option<types::GetTagRecheck>,
    ) -> Result<ResponseValue<types::Tag>, Error<()>> {
        let url = format!("{}/tag/{}", self.baseurl, encode_path(&uuid.to_string()),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new("muted", &muted))
            .query(&progenitor_client::QueryParam::new("recheck", &recheck))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_tag",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            404u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update tag
    ///
    ///Update an existing tag using JSON
    ///
    ///Sends a `PUT` request to `/tag/{uuid}`
    ///
    ///Arguments:
    /// - `uuid`: Tag unique ID
    /// - `body`
    pub async fn update_tag<'a>(
        &'a self,
        uuid: &'a ::uuid::Uuid,
        body: &'a types::Tag,
    ) -> Result<ResponseValue<()>, Error<()>> {
        let url = format!("{}/tag/{}", self.baseurl, encode_path(&uuid.to_string()),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .put(url)
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "update_tag",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::empty(response)),
            500u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete tag
    ///
    ///Delete a tag/group and remove it from all web page change monitors
    /// (watches)
    ///
    ///Sends a `DELETE` request to `/tag/{uuid}`
    ///
    ///Arguments:
    /// - `uuid`: Tag unique ID
    pub async fn delete_tag<'a>(
        &'a self,
        uuid: &'a ::uuid::Uuid,
    ) -> Result<ResponseValue<()>, Error<()>> {
        let url = format!("{}/tag/{}", self.baseurl, encode_path(&uuid.to_string()),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self.client.delete(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "delete_tag",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::empty(response)),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get notification URLs
    ///
    ///Return the notification URL list from the configuration
    ///
    ///Sends a `GET` request to `/notifications`
    pub async fn get_notifications<'a>(
        &'a self,
    ) -> Result<ResponseValue<types::NotificationUrls>, Error<()>> {
        let url = format!("{}/notifications", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_notifications",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Replace notification URLs
    ///
    ///Replace all notification URLs with the provided list (can be empty)
    ///
    ///Sends a `PUT` request to `/notifications`
    pub async fn replace_notifications<'a>(
        &'a self,
        body: &'a types::NotificationUrls,
    ) -> Result<ResponseValue<types::NotificationUrls>, Error<()>> {
        let url = format!("{}/notifications", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .put(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "replace_notifications",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Add notification URLs
    ///
    ///Add one or more notification URLs to the configuration
    ///
    ///Sends a `POST` request to `/notifications`
    pub async fn add_notifications<'a>(
        &'a self,
        body: &'a types::NotificationUrls,
    ) -> Result<ResponseValue<types::NotificationUrls>, Error<()>> {
        let url = format!("{}/notifications", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "add_notifications",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            201u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete notification URLs
    ///
    ///Delete one or more notification URLs from the configuration
    ///
    ///Sends a `DELETE` request to `/notifications`
    pub async fn delete_notifications<'a>(
        &'a self,
        body: &'a types::NotificationUrls,
    ) -> Result<ResponseValue<()>, Error<()>> {
        let url = format!("{}/notifications", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .delete(url)
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "delete_notifications",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            204u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Search watches
    ///
    ///Search web page change monitors (watches) by URL or title text
    ///
    ///Sends a `GET` request to `/search`
    ///
    ///Arguments:
    /// - `partial`: Allow partial matching of URL query
    /// - `q`: Search query to match against watch URLs and titles
    /// - `tag`: Tag name to limit results (name not UUID)
    pub async fn search_watches<'a>(
        &'a self,
        partial: Option<&'a str>,
        q: &'a str,
        tag: Option<&'a str>,
    ) -> Result<ResponseValue<types::SearchResult>, Error<()>> {
        let url = format!("{}/search", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new("partial", &partial))
            .query(&progenitor_client::QueryParam::new("q", &q))
            .query(&progenitor_client::QueryParam::new("tag", &tag))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "search_watches",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Import watch URLs with configuration
    ///
    ///Import a list of URLs to monitor with optional watch configuration.
    /// Accepts line-separated URLs in request body.
    ///
    ///**Configuration via Query Parameters:**
    ///
    ///You can pass ANY watch configuration field as query parameters to apply
    /// settings to all imported watches. All parameters from the Watch
    /// schema are supported (processor, fetch_backend, notification_urls,
    /// etc.).
    ///
    ///**Special Parameters:**
    /// - `tag` / `tag_uuids` - Assign tags to imported watches
    /// - `proxy` - Use specific proxy for imported watches
    /// - `dedupe` - Skip duplicate URLs (default: true)
    ///
    ///**Type Conversion:**
    /// - Booleans: `true`, `false`, `1`, `0`, `yes`, `no`
    /// - Arrays: Comma-separated or JSON format (`[item1,item2]`)
    /// - Objects: JSON format (`{"key":"value"}`)
    /// - Numbers: Parsed as int or float
    ///
    ///
    ///Sends a `POST` request to `/import`
    ///
    ///Arguments:
    /// - `dedupe`: Skip duplicate URLs (default true)
    /// - `proxy`: Proxy key to use for imported watches
    /// - `tag`: Tag name to apply to imported watches
    /// - `tag_uuids`: Tag UUID(s) to apply to imported watches (comma-separated
    ///   for multiple)
    /// - `body`
    pub async fn import_watches<'a>(
        &'a self,
        dedupe: Option<bool>,
        proxy: Option<&'a str>,
        tag: Option<&'a str>,
        tag_uuids: Option<&'a str>,
        body: String,
    ) -> Result<ResponseValue<::std::vec::Vec<::uuid::Uuid>>, Error<()>> {
        let url = format!("{}/import", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .header(
                ::reqwest::header::CONTENT_TYPE,
                ::reqwest::header::HeaderValue::from_static("text/plain"),
            )
            .body(body)
            .query(&progenitor_client::QueryParam::new("dedupe", &dedupe))
            .query(&progenitor_client::QueryParam::new("proxy", &proxy))
            .query(&progenitor_client::QueryParam::new("tag", &tag))
            .query(&progenitor_client::QueryParam::new("tag_uuids", &tag_uuids))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "import_watches",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            500u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get system information
    ///
    ///Return information about the current system state
    ///
    ///Sends a `GET` request to `/systeminfo`
    pub async fn get_system_info<'a>(
        &'a self,
    ) -> Result<ResponseValue<types::SystemInfo>, Error<()>> {
        let url = format!("{}/systeminfo", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_system_info",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get full live API spec
    ///
    ///Return the fully merged OpenAPI specification for this instance.
    ///
    ///Unlike the static `api-spec.yaml` shipped with the application, this
    /// endpoint returns the spec dynamically merged with any `api.yaml`
    /// schemas provided by installed processor plugins.
    ///
    ///**Use this URL** with Swagger UI or Redoc to get schema-accurate
    /// documentation for your specific install — it includes every
    /// `processor_config_<name>` schema block contributed by
    /// installed processors (e.g. `processor_config_restock_diff` from the
    /// built-in restock plugin).
    ///
    ///This endpoint requires no authentication and returns YAML.
    ///
    ///To load it directly in Swagger UI, paste the URL into the "Explore" box:
    ///```
    /// http://localhost:5000/api/v1/full-spec
    /// ```
    ///
    ///
    ///Sends a `GET` request to `/full-spec`
    pub async fn get_full_api_spec<'a>(&'a self) -> Result<ResponseValue<ByteStream>, Error<()>> {
        let url = format!("{}/full-spec", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self.client.get(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "get_full_api_spec",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::stream(response)),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
}

/// Items consumers will typically use such as the Client.
pub mod prelude {
    #[allow(unused_imports)]
    pub use super::Client;
}
