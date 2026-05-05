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

    ///Direction to order results.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Direction to order results.",
    ///  "examples": [
    ///    "desc"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "asc",
    ///    "desc"
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
    pub enum AccountsListAccountsDirection {
        #[serde(rename = "asc")]
        Asc,
        #[serde(rename = "desc")]
        Desc,
    }

    impl ::std::fmt::Display for AccountsListAccountsDirection {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Asc => f.write_str("asc"),
                Self::Desc => f.write_str("desc"),
            }
        }
    }

    impl ::std::str::FromStr for AccountsListAccountsDirection {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "asc" => Ok(Self::Asc),
                "desc" => Ok(Self::Desc),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for AccountsListAccountsDirection {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AccountsListAccountsDirection {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AccountsListAccountsDirection {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`CacheApiResponseCommonFailure`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "result",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "examples": [
    ///        [
    ///          {
    ///            "code": 7003,
    ///            "message": "No route for the URI"
    ///          }
    ///        ]
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/cache_messages"
    ///        }
    ///      ],
    ///      "minLength": 1
    ///    },
    ///    "messages": {
    ///      "examples": [
    ///        []
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/cache_messages"
    ///        }
    ///      ]
    ///    },
    ///    "result": {
    ///      "$ref": "#/components/schemas/cache_result"
    ///    },
    ///    "success": {
    ///      "description": "Indicates the API call's success or failure.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CacheApiResponseCommonFailure {
        pub errors: ::std::vec::Vec<CacheApiResponseCommonFailureErrorsItem>,
        pub messages: CacheMessages,
        pub result: CacheResult,
        ///Indicates the API call's success or failure.
        pub success: bool,
    }

    ///`CacheApiResponseCommonFailureErrorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CacheApiResponseCommonFailureErrorsItem {
        pub code: i64,
        pub message: ::std::string::String,
    }

    ///`CacheApiResponseSingleId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "result",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "$ref": "#/components/schemas/cache_messages"
    ///    },
    ///    "messages": {
    ///      "$ref": "#/components/schemas/cache_messages"
    ///    },
    ///    "result": {
    ///      "$ref": "#/components/schemas/cache_result"
    ///    },
    ///    "success": {
    ///      "description": "Indicates the API call's success or failure.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CacheApiResponseSingleId {
        pub errors: CacheMessages,
        pub messages: CacheMessages,
        pub result: CacheResult,
        ///Indicates the API call's success or failure.
        pub success: bool,
    }

    ///`CacheIdentifier`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    "023e105f4ecef8ad9ca31a8372d0c353"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CacheIdentifier(::std::string::String);
    impl ::std::ops::Deref for CacheIdentifier {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CacheIdentifier> for ::std::string::String {
        fn from(value: CacheIdentifier) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CacheIdentifier {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CacheIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CacheIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CacheIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CacheIdentifier {
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

    ///`CacheMessages`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    []
    ///  ],
    ///  "type": "array",
    ///  "items": {
    ///    "type": "object",
    ///    "uniqueItems": true,
    ///    "required": [
    ///      "code",
    ///      "message"
    ///    ],
    ///    "properties": {
    ///      "code": {
    ///        "type": "integer",
    ///        "minimum": 1000.0
    ///      },
    ///      "message": {
    ///        "type": "string"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct CacheMessages(pub ::std::vec::Vec<CacheMessagesItem>);
    impl ::std::ops::Deref for CacheMessages {
        type Target = ::std::vec::Vec<CacheMessagesItem>;
        fn deref(&self) -> &::std::vec::Vec<CacheMessagesItem> {
            &self.0
        }
    }

    impl ::std::convert::From<CacheMessages> for ::std::vec::Vec<CacheMessagesItem> {
        fn from(value: CacheMessages) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::vec::Vec<CacheMessagesItem>> for CacheMessages {
        fn from(value: ::std::vec::Vec<CacheMessagesItem>) -> Self {
            Self(value)
        }
    }

    ///`CacheMessagesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CacheMessagesItem {
        pub code: i64,
        pub message: ::std::string::String,
    }

    ///`CacheResult`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "editable",
    ///    "id",
    ///    "modified_on",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "editable": {
    ///      "description": "Whether this setting can be updated or not.",
    ///      "readOnly": true,
    ///      "type": "boolean"
    ///    },
    ///    "id": {
    ///      "readOnly": true,
    ///      "examples": [
    ///        "ssl_automatic_mode"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "modified_on": {
    ///      "description": "Last time this setting was modified.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "2014-01-01T05:20:00.12345Z"
    ///      ],
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "next_scheduled_scan": {
    ///      "description": "Next time this zone will be scanned by the
    /// Automatic SSL/TLS.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "2014-01-01T05:20:00.12345Z"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "format": "date-time"
    ///    },
    ///    "value": {
    ///      "description": "Current setting of the automatic SSL/TLS.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "auto"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "auto",
    ///        "custom"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CacheResult {
        ///Whether this setting can be updated or not.
        pub editable: bool,
        pub id: ::std::string::String,
        ///Last time this setting was modified.
        pub modified_on: ::chrono::DateTime<::chrono::offset::Utc>,
        ///Next time this zone will be scanned by the Automatic SSL/TLS.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub next_scheduled_scan: ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
        ///Current setting of the automatic SSL/TLS.
        pub value: CacheResultValue,
    }

    ///Current setting of the automatic SSL/TLS.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Current setting of the automatic SSL/TLS.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "auto"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "auto",
    ///    "custom"
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
    pub enum CacheResultValue {
        #[serde(rename = "auto")]
        Auto,
        #[serde(rename = "custom")]
        Custom,
    }

    impl ::std::fmt::Display for CacheResultValue {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Auto => f.write_str("auto"),
                Self::Custom => f.write_str("custom"),
            }
        }
    }

    impl ::std::str::FromStr for CacheResultValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "auto" => Ok(Self::Auto),
                "custom" => Ok(Self::Custom),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CacheResultValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CacheResultValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CacheResultValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Update enablement of Automatic SSL/TLS.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Update enablement of Automatic SSL/TLS.",
    ///  "type": "object",
    ///  "required": [
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "value": {
    ///      "$ref": "#/components/schemas/cache_schemas_value"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CacheSchemasPatch {
        pub value: CacheSchemasValue,
    }

    ///Controls enablement of Automatic SSL/TLS.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Controls enablement of Automatic SSL/TLS.",
    ///  "examples": [
    ///    "auto"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "auto",
    ///    "custom"
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
    pub enum CacheSchemasValue {
        #[serde(rename = "auto")]
        Auto,
        #[serde(rename = "custom")]
        Custom,
    }

    impl ::std::fmt::Display for CacheSchemasValue {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Auto => f.write_str("auto"),
                Self::Custom => f.write_str("custom"),
            }
        }
    }

    impl ::std::str::FromStr for CacheSchemasValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "auto" => Ok(Self::Auto),
                "custom" => Ok(Self::Custom),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CacheSchemasValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CacheSchemasValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CacheSchemasValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///DNS record type (simplified). Original: dns-records_ARecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_ARecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsARecord(pub ::serde_json::Map<::std::string::String, ::serde_json::Value>);
    impl ::std::ops::Deref for DnsRecordsARecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsARecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsARecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsARecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///DNS record type (simplified). Original: dns-records_AAAARecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_AAAARecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsAaaaRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsAaaaRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsAaaaRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsAaaaRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsAaaaRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///`DnsRecordsApiResponseCollection`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dns-records_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result_info": {
    ///          "type": "object",
    ///          "properties": {
    ///            "count": {
    ///              "description": "Total number of results for the requested
    /// service.",
    ///              "examples": [
    ///                1
    ///              ],
    ///              "type": "number"
    ///            },
    ///            "page": {
    ///              "description": "Current page within paginated list of
    /// results.",
    ///              "examples": [
    ///                1
    ///              ],
    ///              "type": "number"
    ///            },
    ///            "per_page": {
    ///              "description": "Number of results per page of results.",
    ///              "examples": [
    ///                20
    ///              ],
    ///              "type": "number"
    ///            },
    ///            "total_count": {
    ///              "description": "Total results available without any search
    /// parameters.",
    ///              "examples": [
    ///                2000
    ///              ],
    ///              "type": "number"
    ///            },
    ///            "total_pages": {
    ///              "description": "The number of total pages in the entire
    /// result set.",
    ///              "examples": [
    ///                100
    ///              ],
    ///              "type": "number"
    ///            }
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsApiResponseCollection {
        pub errors: DnsRecordsMessages,
        pub messages: DnsRecordsMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result_info: ::std::option::Option<DnsRecordsApiResponseCollectionResultInfo>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`DnsRecordsApiResponseCollectionResultInfo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "count": {
    ///      "description": "Total number of results for the requested
    /// service.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "page": {
    ///      "description": "Current page within paginated list of results.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "per_page": {
    ///      "description": "Number of results per page of results.",
    ///      "examples": [
    ///        20
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_count": {
    ///      "description": "Total results available without any search
    /// parameters.",
    ///      "examples": [
    ///        2000
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_pages": {
    ///      "description": "The number of total pages in the entire result
    /// set.",
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsApiResponseCollectionResultInfo {
        ///Total number of results for the requested service.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub count: ::std::option::Option<f64>,
        ///Current page within paginated list of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub page: ::std::option::Option<f64>,
        ///Number of results per page of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub per_page: ::std::option::Option<f64>,
        ///Total results available without any search parameters.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_count: ::std::option::Option<f64>,
        ///The number of total pages in the entire result set.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_pages: ::std::option::Option<f64>,
    }

    impl ::std::default::Default for DnsRecordsApiResponseCollectionResultInfo {
        fn default() -> Self {
            Self {
                count: Default::default(),
                page: Default::default(),
                per_page: Default::default(),
                total_count: Default::default(),
                total_pages: Default::default(),
            }
        }
    }

    ///`DnsRecordsApiResponseCommon`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "$ref": "#/components/schemas/dns-records_messages"
    ///    },
    ///    "messages": {
    ///      "$ref": "#/components/schemas/dns-records_messages"
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "enum": [
    ///        true
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsApiResponseCommon {
        pub errors: DnsRecordsMessages,
        pub messages: DnsRecordsMessages,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`DnsRecordsApiResponseCommonFailure`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "result",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "examples": [
    ///        [
    ///          {
    ///            "code": 7003,
    ///            "message": "No route for the URI"
    ///          }
    ///        ]
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/dns-records_messages"
    ///        }
    ///      ],
    ///      "minLength": 1
    ///    },
    ///    "messages": {
    ///      "examples": [
    ///        []
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/dns-records_messages"
    ///        }
    ///      ]
    ///    },
    ///    "result": {
    ///      "type": [
    ///        "object",
    ///        "null"
    ///      ],
    ///      "enum": [
    ///        null
    ///      ]
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean",
    ///      "enum": [
    ///        false
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsApiResponseCommonFailure {
        pub errors: ::std::vec::Vec<DnsRecordsApiResponseCommonFailureErrorsItem>,
        pub messages: DnsRecordsMessages,
        pub result: (),
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`DnsRecordsApiResponseCommonFailureErrorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "documentation_url": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "source": {
    ///      "type": "object",
    ///      "properties": {
    ///        "pointer": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsApiResponseCommonFailureErrorsItem {
        pub code: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub documentation_url: ::std::option::Option<::std::string::String>,
        pub message: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<DnsRecordsApiResponseCommonFailureErrorsItemSource>,
    }

    ///`DnsRecordsApiResponseCommonFailureErrorsItemSource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "pointer": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsApiResponseCommonFailureErrorsItemSource {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pointer: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for DnsRecordsApiResponseCommonFailureErrorsItemSource {
        fn default() -> Self {
            Self {
                pointer: Default::default(),
            }
        }
    }

    ///`DnsRecordsApiResponseSingle`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dns-records_api-response-common"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsApiResponseSingle(pub DnsRecordsApiResponseCommon);
    impl ::std::ops::Deref for DnsRecordsApiResponseSingle {
        type Target = DnsRecordsApiResponseCommon;
        fn deref(&self) -> &DnsRecordsApiResponseCommon {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsApiResponseSingle> for DnsRecordsApiResponseCommon {
        fn from(value: DnsRecordsApiResponseSingle) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<DnsRecordsApiResponseCommon> for DnsRecordsApiResponseSingle {
        fn from(value: DnsRecordsApiResponseCommon) -> Self {
            Self(value)
        }
    }

    ///DNS record type (simplified). Original: dns-records_CAARecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_CAARecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsCaaRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsCaaRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsCaaRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsCaaRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsCaaRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///DNS record type (simplified). Original: dns-records_CERTRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_CERTRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsCertRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsCertRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsCertRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsCertRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsCertRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///DNS record type (simplified). Original: dns-records_CNAMERecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_CNAMERecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsCnameRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsCnameRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsCnameRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsCnameRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsCnameRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///Comments or notes about the DNS record. This field has no effect on DNS
    /// responses.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Comments or notes about the DNS record. This field has
    /// no effect on DNS responses.",
    ///  "examples": [
    ///    "Domain verification record"
    ///  ],
    ///  "type": "string",
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    #[serde(transparent)]
    pub struct DnsRecordsComment(pub ::std::string::String);
    impl ::std::ops::Deref for DnsRecordsComment {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsComment> for ::std::string::String {
        fn from(value: DnsRecordsComment) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::string::String> for DnsRecordsComment {
        fn from(value: ::std::string::String) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DnsRecordsComment {
        type Err = ::std::convert::Infallible;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::fmt::Display for DnsRecordsComment {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Direction to order DNS records in.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Direction to order DNS records in.",
    ///  "default": "asc",
    ///  "type": "string",
    ///  "enum": [
    ///    "asc",
    ///    "desc"
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
    pub enum DnsRecordsDirection {
        #[serde(rename = "asc")]
        Asc,
        #[serde(rename = "desc")]
        Desc,
    }

    impl ::std::fmt::Display for DnsRecordsDirection {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Asc => f.write_str("asc"),
                Self::Desc => f.write_str("desc"),
            }
        }
    }

    impl ::std::str::FromStr for DnsRecordsDirection {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "asc" => Ok(Self::Asc),
                "desc" => Ok(Self::Desc),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsDirection {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DnsRecordsDirection {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DnsRecordsDirection {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for DnsRecordsDirection {
        fn default() -> Self {
            DnsRecordsDirection::Asc
        }
    }

    ///DNS record (simplified from complex union). Original schema:
    /// dns-records_dns-record-patch
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record (simplified from complex union). Original
    /// schema: dns-records_dns-record-patch",
    ///  "type": "object",
    ///  "properties": {
    ///    "comment": {
    ///      "type": "string"
    ///    },
    ///    "content": {
    ///      "description": "Record content/value",
    ///      "type": "string"
    ///    },
    ///    "created_on": {
    ///      "type": "string"
    ///    },
    ///    "data": {
    ///      "description": "Structured data for complex record types",
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "modified_on": {
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "description": "Record name",
    ///      "type": "string"
    ///    },
    ///    "priority": {
    ///      "description": "Priority for MX/SRV",
    ///      "type": "number"
    ///    },
    ///    "proxiable": {
    ///      "type": "boolean"
    ///    },
    ///    "proxied": {
    ///      "type": "boolean"
    ///    },
    ///    "tags": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "ttl": {
    ///      "description": "TTL in seconds, 1=auto",
    ///      "type": "number"
    ///    },
    ///    "type": {
    ///      "description": "Record type (A, AAAA, CNAME, etc.)",
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsDnsRecordPatch {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub comment: ::std::option::Option<::std::string::String>,
        ///Record content/value
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub content: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub created_on: ::std::option::Option<::std::string::String>,
        ///Structured data for complex record types
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub data: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub modified_on: ::std::option::Option<::std::string::String>,
        ///Record name
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        ///Priority for MX/SRV
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub priority: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxiable: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxied: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///TTL in seconds, 1=auto
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<f64>,
        ///Record type (A, AAAA, CNAME, etc.)
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for DnsRecordsDnsRecordPatch {
        fn default() -> Self {
            Self {
                comment: Default::default(),
                content: Default::default(),
                created_on: Default::default(),
                data: Default::default(),
                id: Default::default(),
                modified_on: Default::default(),
                name: Default::default(),
                priority: Default::default(),
                proxiable: Default::default(),
                proxied: Default::default(),
                tags: Default::default(),
                ttl: Default::default(),
                type_: Default::default(),
            }
        }
    }

    ///DNS record (simplified from complex union). Original schema:
    /// dns-records_dns-record-post
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record (simplified from complex union). Original
    /// schema: dns-records_dns-record-post",
    ///  "type": "object",
    ///  "properties": {
    ///    "comment": {
    ///      "type": "string"
    ///    },
    ///    "content": {
    ///      "description": "Record content/value",
    ///      "type": "string"
    ///    },
    ///    "created_on": {
    ///      "type": "string"
    ///    },
    ///    "data": {
    ///      "description": "Structured data for complex record types",
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "modified_on": {
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "description": "Record name",
    ///      "type": "string"
    ///    },
    ///    "priority": {
    ///      "description": "Priority for MX/SRV",
    ///      "type": "number"
    ///    },
    ///    "proxiable": {
    ///      "type": "boolean"
    ///    },
    ///    "proxied": {
    ///      "type": "boolean"
    ///    },
    ///    "tags": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "ttl": {
    ///      "description": "TTL in seconds, 1=auto",
    ///      "type": "number"
    ///    },
    ///    "type": {
    ///      "description": "Record type (A, AAAA, CNAME, etc.)",
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsDnsRecordPost {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub comment: ::std::option::Option<::std::string::String>,
        ///Record content/value
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub content: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub created_on: ::std::option::Option<::std::string::String>,
        ///Structured data for complex record types
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub data: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub modified_on: ::std::option::Option<::std::string::String>,
        ///Record name
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        ///Priority for MX/SRV
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub priority: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxiable: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxied: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///TTL in seconds, 1=auto
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<f64>,
        ///Record type (A, AAAA, CNAME, etc.)
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for DnsRecordsDnsRecordPost {
        fn default() -> Self {
            Self {
                comment: Default::default(),
                content: Default::default(),
                created_on: Default::default(),
                data: Default::default(),
                id: Default::default(),
                modified_on: Default::default(),
                name: Default::default(),
                priority: Default::default(),
                proxiable: Default::default(),
                proxied: Default::default(),
                tags: Default::default(),
                ttl: Default::default(),
                type_: Default::default(),
            }
        }
    }

    ///DNS record (simplified from complex union). Original schema:
    /// dns-records_dns-record-response
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record (simplified from complex union). Original
    /// schema: dns-records_dns-record-response",
    ///  "type": "object",
    ///  "properties": {
    ///    "comment": {
    ///      "type": "string"
    ///    },
    ///    "content": {
    ///      "description": "Record content/value",
    ///      "type": "string"
    ///    },
    ///    "created_on": {
    ///      "type": "string"
    ///    },
    ///    "data": {
    ///      "description": "Structured data for complex record types",
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "modified_on": {
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "description": "Record name",
    ///      "type": "string"
    ///    },
    ///    "priority": {
    ///      "description": "Priority for MX/SRV",
    ///      "type": "number"
    ///    },
    ///    "proxiable": {
    ///      "type": "boolean"
    ///    },
    ///    "proxied": {
    ///      "type": "boolean"
    ///    },
    ///    "tags": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "ttl": {
    ///      "description": "TTL in seconds, 1=auto",
    ///      "type": "number"
    ///    },
    ///    "type": {
    ///      "description": "Record type (A, AAAA, CNAME, etc.)",
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsDnsRecordResponse {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub comment: ::std::option::Option<::std::string::String>,
        ///Record content/value
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub content: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub created_on: ::std::option::Option<::std::string::String>,
        ///Structured data for complex record types
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub data: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub modified_on: ::std::option::Option<::std::string::String>,
        ///Record name
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        ///Priority for MX/SRV
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub priority: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxiable: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxied: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///TTL in seconds, 1=auto
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<f64>,
        ///Record type (A, AAAA, CNAME, etc.)
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for DnsRecordsDnsRecordResponse {
        fn default() -> Self {
            Self {
                comment: Default::default(),
                content: Default::default(),
                created_on: Default::default(),
                data: Default::default(),
                id: Default::default(),
                modified_on: Default::default(),
                name: Default::default(),
                priority: Default::default(),
                proxiable: Default::default(),
                proxied: Default::default(),
                tags: Default::default(),
                ttl: Default::default(),
                type_: Default::default(),
            }
        }
    }

    ///DNS record (simplified from complex union). Original schema:
    /// dns-records_dns-record-shared-fields
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record (simplified from complex union). Original
    /// schema: dns-records_dns-record-shared-fields",
    ///  "type": "object",
    ///  "properties": {
    ///    "comment": {
    ///      "type": "string"
    ///    },
    ///    "content": {
    ///      "description": "Record content/value",
    ///      "type": "string"
    ///    },
    ///    "created_on": {
    ///      "type": "string"
    ///    },
    ///    "data": {
    ///      "description": "Structured data for complex record types",
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "modified_on": {
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "description": "Record name",
    ///      "type": "string"
    ///    },
    ///    "priority": {
    ///      "description": "Priority for MX/SRV",
    ///      "type": "number"
    ///    },
    ///    "proxiable": {
    ///      "type": "boolean"
    ///    },
    ///    "proxied": {
    ///      "type": "boolean"
    ///    },
    ///    "tags": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "ttl": {
    ///      "description": "TTL in seconds, 1=auto",
    ///      "type": "number"
    ///    },
    ///    "type": {
    ///      "description": "Record type (A, AAAA, CNAME, etc.)",
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsDnsRecordSharedFields {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub comment: ::std::option::Option<::std::string::String>,
        ///Record content/value
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub content: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub created_on: ::std::option::Option<::std::string::String>,
        ///Structured data for complex record types
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub data: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub modified_on: ::std::option::Option<::std::string::String>,
        ///Record name
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        ///Priority for MX/SRV
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub priority: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxiable: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxied: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///TTL in seconds, 1=auto
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<f64>,
        ///Record type (A, AAAA, CNAME, etc.)
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for DnsRecordsDnsRecordSharedFields {
        fn default() -> Self {
            Self {
                comment: Default::default(),
                content: Default::default(),
                created_on: Default::default(),
                data: Default::default(),
                id: Default::default(),
                modified_on: Default::default(),
                name: Default::default(),
                priority: Default::default(),
                proxiable: Default::default(),
                proxied: Default::default(),
                tags: Default::default(),
                ttl: Default::default(),
                type_: Default::default(),
            }
        }
    }

    ///DNS record (simplified from complex union). Original schema:
    /// dns-records_dns-record-with-data
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record (simplified from complex union). Original
    /// schema: dns-records_dns-record-with-data",
    ///  "type": "object",
    ///  "properties": {
    ///    "comment": {
    ///      "type": "string"
    ///    },
    ///    "content": {
    ///      "description": "Record content/value",
    ///      "type": "string"
    ///    },
    ///    "created_on": {
    ///      "type": "string"
    ///    },
    ///    "data": {
    ///      "description": "Structured data for complex record types",
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "modified_on": {
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "description": "Record name",
    ///      "type": "string"
    ///    },
    ///    "priority": {
    ///      "description": "Priority for MX/SRV",
    ///      "type": "number"
    ///    },
    ///    "proxiable": {
    ///      "type": "boolean"
    ///    },
    ///    "proxied": {
    ///      "type": "boolean"
    ///    },
    ///    "tags": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "ttl": {
    ///      "description": "TTL in seconds, 1=auto",
    ///      "type": "number"
    ///    },
    ///    "type": {
    ///      "description": "Record type (A, AAAA, CNAME, etc.)",
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsDnsRecordWithData {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub comment: ::std::option::Option<::std::string::String>,
        ///Record content/value
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub content: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub created_on: ::std::option::Option<::std::string::String>,
        ///Structured data for complex record types
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub data: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub modified_on: ::std::option::Option<::std::string::String>,
        ///Record name
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        ///Priority for MX/SRV
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub priority: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxiable: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxied: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///TTL in seconds, 1=auto
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<f64>,
        ///Record type (A, AAAA, CNAME, etc.)
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for DnsRecordsDnsRecordWithData {
        fn default() -> Self {
            Self {
                comment: Default::default(),
                content: Default::default(),
                created_on: Default::default(),
                data: Default::default(),
                id: Default::default(),
                modified_on: Default::default(),
                name: Default::default(),
                priority: Default::default(),
                proxiable: Default::default(),
                proxied: Default::default(),
                tags: Default::default(),
                ttl: Default::default(),
                type_: Default::default(),
            }
        }
    }

    ///DNS record (simplified from complex union). Original schema:
    /// dns-records_dns-record-without-data
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record (simplified from complex union). Original
    /// schema: dns-records_dns-record-without-data",
    ///  "type": "object",
    ///  "properties": {
    ///    "comment": {
    ///      "type": "string"
    ///    },
    ///    "content": {
    ///      "description": "Record content/value",
    ///      "type": "string"
    ///    },
    ///    "created_on": {
    ///      "type": "string"
    ///    },
    ///    "data": {
    ///      "description": "Structured data for complex record types",
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "modified_on": {
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "description": "Record name",
    ///      "type": "string"
    ///    },
    ///    "priority": {
    ///      "description": "Priority for MX/SRV",
    ///      "type": "number"
    ///    },
    ///    "proxiable": {
    ///      "type": "boolean"
    ///    },
    ///    "proxied": {
    ///      "type": "boolean"
    ///    },
    ///    "tags": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "ttl": {
    ///      "description": "TTL in seconds, 1=auto",
    ///      "type": "number"
    ///    },
    ///    "type": {
    ///      "description": "Record type (A, AAAA, CNAME, etc.)",
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsDnsRecordWithoutData {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub comment: ::std::option::Option<::std::string::String>,
        ///Record content/value
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub content: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub created_on: ::std::option::Option<::std::string::String>,
        ///Structured data for complex record types
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub data: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub modified_on: ::std::option::Option<::std::string::String>,
        ///Record name
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        ///Priority for MX/SRV
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub priority: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxiable: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub proxied: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        ///TTL in seconds, 1=auto
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<f64>,
        ///Record type (A, AAAA, CNAME, etc.)
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for DnsRecordsDnsRecordWithoutData {
        fn default() -> Self {
            Self {
                comment: Default::default(),
                content: Default::default(),
                created_on: Default::default(),
                data: Default::default(),
                id: Default::default(),
                modified_on: Default::default(),
                name: Default::default(),
                priority: Default::default(),
                proxiable: Default::default(),
                proxied: Default::default(),
                tags: Default::default(),
                ttl: Default::default(),
                type_: Default::default(),
            }
        }
    }

    ///`DnsRecordsDnsResponseCollection`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dns-records_api-response-collection"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "type": "array",
    ///          "items": {
    ///            "$ref":
    /// "#/components/schemas/dns-records_dns-record-response"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsDnsResponseCollection {
        pub errors: DnsRecordsMessages,
        pub messages: DnsRecordsMessages,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub result: ::std::vec::Vec<DnsRecordsDnsRecordResponse>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result_info: ::std::option::Option<DnsRecordsDnsResponseCollectionResultInfo>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`DnsRecordsDnsResponseCollectionResultInfo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "count": {
    ///      "description": "Total number of results for the requested
    /// service.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "page": {
    ///      "description": "Current page within paginated list of results.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "per_page": {
    ///      "description": "Number of results per page of results.",
    ///      "examples": [
    ///        20
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_count": {
    ///      "description": "Total results available without any search
    /// parameters.",
    ///      "examples": [
    ///        2000
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_pages": {
    ///      "description": "The number of total pages in the entire result
    /// set.",
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsDnsResponseCollectionResultInfo {
        ///Total number of results for the requested service.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub count: ::std::option::Option<f64>,
        ///Current page within paginated list of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub page: ::std::option::Option<f64>,
        ///Number of results per page of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub per_page: ::std::option::Option<f64>,
        ///Total results available without any search parameters.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_count: ::std::option::Option<f64>,
        ///The number of total pages in the entire result set.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_pages: ::std::option::Option<f64>,
    }

    impl ::std::default::Default for DnsRecordsDnsResponseCollectionResultInfo {
        fn default() -> Self {
            Self {
                count: Default::default(),
                page: Default::default(),
                per_page: Default::default(),
                total_count: Default::default(),
                total_pages: Default::default(),
            }
        }
    }

    ///`DnsRecordsDnsResponseSingle`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dns-records_api-response-single"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/dns-records_dns-record-response"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsDnsResponseSingle {
        pub errors: DnsRecordsMessages,
        pub messages: DnsRecordsMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result: ::std::option::Option<DnsRecordsDnsRecordResponse>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///DNS record type (simplified). Original: dns-records_DNSKEYRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_DNSKEYRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsDnskeyRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsDnskeyRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsDnskeyRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsDnskeyRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsDnskeyRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///DNS record type (simplified). Original: dns-records_DSRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_DSRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsDsRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsDsRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsDsRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsDsRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsDsRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///`DnsRecordsForAZoneCreateDnsRecordResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dns-records_dns_response_single"
    ///    },
    ///    {
    ///      "$ref":
    /// "#/components/schemas/dns-records_api-response-common-failure"
    ///    }
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
    #[serde(deny_unknown_fields)]
    pub enum DnsRecordsForAZoneCreateDnsRecordResponse {}
    ///`DnsRecordsForAZoneDeleteDnsRecordResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "type": "object",
    ///          "properties": {
    ///            "id": {
    ///              "$ref": "#/components/schemas/dns-records_identifier"
    ///            }
    ///          }
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "$ref":
    /// "#/components/schemas/dns-records_api-response-common-failure"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsForAZoneDeleteDnsRecordResponse {
        pub errors: ::std::vec::Vec<DnsRecordsForAZoneDeleteDnsRecordResponseErrorsItem>,
        pub messages: DnsRecordsMessages,
        pub result: DnsRecordsForAZoneDeleteDnsRecordResponseResult,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`DnsRecordsForAZoneDeleteDnsRecordResponseErrorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "documentation_url": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "source": {
    ///      "type": "object",
    ///      "properties": {
    ///        "pointer": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsForAZoneDeleteDnsRecordResponseErrorsItem {
        pub code: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub documentation_url: ::std::option::Option<::std::string::String>,
        pub message: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source:
            ::std::option::Option<DnsRecordsForAZoneDeleteDnsRecordResponseErrorsItemSource>,
    }

    ///`DnsRecordsForAZoneDeleteDnsRecordResponseErrorsItemSource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "pointer": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsForAZoneDeleteDnsRecordResponseErrorsItemSource {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pointer: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for DnsRecordsForAZoneDeleteDnsRecordResponseErrorsItemSource {
        fn default() -> Self {
            Self {
                pointer: Default::default(),
            }
        }
    }

    ///`DnsRecordsForAZoneDeleteDnsRecordResponseResult`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "enum": [],
    ///  "properties": {
    ///    "id": {
    ///      "$ref": "#/components/schemas/dns-records_identifier"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsForAZoneDeleteDnsRecordResponseResult(
        DnsRecordsForAZoneDeleteDnsRecordResponseResultInner,
    );
    impl ::std::ops::Deref for DnsRecordsForAZoneDeleteDnsRecordResponseResult {
        type Target = DnsRecordsForAZoneDeleteDnsRecordResponseResultInner;
        fn deref(&self) -> &DnsRecordsForAZoneDeleteDnsRecordResponseResultInner {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsForAZoneDeleteDnsRecordResponseResult>
        for DnsRecordsForAZoneDeleteDnsRecordResponseResultInner
    {
        fn from(value: DnsRecordsForAZoneDeleteDnsRecordResponseResult) -> Self {
            value.0
        }
    }

    impl ::std::convert::TryFrom<DnsRecordsForAZoneDeleteDnsRecordResponseResultInner>
        for DnsRecordsForAZoneDeleteDnsRecordResponseResult
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: DnsRecordsForAZoneDeleteDnsRecordResponseResultInner,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if ![].contains(&value) {
                Err("invalid value".into())
            } else {
                Ok(Self(value))
            }
        }
    }

    impl<'de> ::serde::Deserialize<'de> for DnsRecordsForAZoneDeleteDnsRecordResponseResult {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            Self::try_from(
                <DnsRecordsForAZoneDeleteDnsRecordResponseResultInner>::deserialize(deserializer)?,
            )
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
        }
    }

    ///`DnsRecordsForAZoneDeleteDnsRecordResponseResultInner`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "enum": [],
    ///  "properties": {
    ///    "id": {
    ///      "$ref": "#/components/schemas/dns-records_identifier"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, PartialEq)]
    pub struct DnsRecordsForAZoneDeleteDnsRecordResponseResultInner {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<DnsRecordsIdentifier>,
    }

    impl ::std::default::Default for DnsRecordsForAZoneDeleteDnsRecordResponseResultInner {
        fn default() -> Self {
            Self {
                id: Default::default(),
            }
        }
    }

    ///`DnsRecordsForAZoneDnsRecordDetailsResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dns-records_dns_response_single"
    ///    },
    ///    {
    ///      "$ref":
    /// "#/components/schemas/dns-records_api-response-common-failure"
    ///    }
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
    #[serde(deny_unknown_fields)]
    pub enum DnsRecordsForAZoneDnsRecordDetailsResponse {}
    ///`DnsRecordsForAZoneListDnsRecordsResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dns-records_dns_response_collection"
    ///    },
    ///    {
    ///      "$ref":
    /// "#/components/schemas/dns-records_api-response-common-failure"
    ///    }
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
    #[serde(deny_unknown_fields)]
    pub enum DnsRecordsForAZoneListDnsRecordsResponse {}
    ///`DnsRecordsForAZonePatchDnsRecordResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dns-records_dns_response_single"
    ///    },
    ///    {
    ///      "$ref":
    /// "#/components/schemas/dns-records_api-response-common-failure"
    ///    }
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
    #[serde(deny_unknown_fields)]
    pub enum DnsRecordsForAZonePatchDnsRecordResponse {}
    ///`DnsRecordsForAZoneUpdateDnsRecordResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dns-records_dns_response_single"
    ///    },
    ///    {
    ///      "$ref":
    /// "#/components/schemas/dns-records_api-response-common-failure"
    ///    }
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
    #[serde(deny_unknown_fields)]
    pub enum DnsRecordsForAZoneUpdateDnsRecordResponse {}
    ///DNS record type (simplified). Original: dns-records_HTTPSRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_HTTPSRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsHttpsRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsHttpsRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsHttpsRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsHttpsRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsHttpsRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///Identifier.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Identifier.",
    ///  "examples": [
    ///    "023e105f4ecef8ad9ca31a8372d0c353"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32,
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct DnsRecordsIdentifier(::std::string::String);
    impl ::std::ops::Deref for DnsRecordsIdentifier {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsIdentifier> for ::std::string::String {
        fn from(value: DnsRecordsIdentifier) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for DnsRecordsIdentifier {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DnsRecordsIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DnsRecordsIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for DnsRecordsIdentifier {
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

    ///DNS record type (simplified). Original: dns-records_LOCRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_LOCRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsLocRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsLocRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsLocRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsLocRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsLocRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///Whether to match all search requirements or at least one (any). If set
    /// to `all`, acts like a logical AND between filters. If set to `any`, acts
    /// like a logical OR instead. Note that the interaction between tag filters
    /// is controlled by the `tag-match` parameter instead.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Whether to match all search requirements or at least
    /// one (any). If set to `all`, acts like a logical AND between filters. If
    /// set to `any`, acts like a logical OR instead. Note that the interaction
    /// between tag filters is controlled by the `tag-match` parameter
    /// instead.\n",
    ///  "default": "all",
    ///  "examples": [
    ///    "any"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "any",
    ///    "all"
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
    pub enum DnsRecordsMatch {
        #[serde(rename = "any")]
        Any,
        #[serde(rename = "all")]
        All,
    }

    impl ::std::fmt::Display for DnsRecordsMatch {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Any => f.write_str("any"),
                Self::All => f.write_str("all"),
            }
        }
    }

    impl ::std::str::FromStr for DnsRecordsMatch {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "any" => Ok(Self::Any),
                "all" => Ok(Self::All),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsMatch {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DnsRecordsMatch {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DnsRecordsMatch {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for DnsRecordsMatch {
        fn default() -> Self {
            DnsRecordsMatch::All
        }
    }

    ///`DnsRecordsMessages`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    []
    ///  ],
    ///  "type": "array",
    ///  "items": {
    ///    "type": "object",
    ///    "uniqueItems": true,
    ///    "required": [
    ///      "code",
    ///      "message"
    ///    ],
    ///    "properties": {
    ///      "code": {
    ///        "type": "integer",
    ///        "minimum": 1000.0
    ///      },
    ///      "documentation_url": {
    ///        "type": "string"
    ///      },
    ///      "message": {
    ///        "type": "string"
    ///      },
    ///      "source": {
    ///        "type": "object",
    ///        "properties": {
    ///          "pointer": {
    ///            "type": "string"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsMessages(pub ::std::vec::Vec<DnsRecordsMessagesItem>);
    impl ::std::ops::Deref for DnsRecordsMessages {
        type Target = ::std::vec::Vec<DnsRecordsMessagesItem>;
        fn deref(&self) -> &::std::vec::Vec<DnsRecordsMessagesItem> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsMessages> for ::std::vec::Vec<DnsRecordsMessagesItem> {
        fn from(value: DnsRecordsMessages) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::vec::Vec<DnsRecordsMessagesItem>> for DnsRecordsMessages {
        fn from(value: ::std::vec::Vec<DnsRecordsMessagesItem>) -> Self {
            Self(value)
        }
    }

    ///`DnsRecordsMessagesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "documentation_url": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "source": {
    ///      "type": "object",
    ///      "properties": {
    ///        "pointer": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsMessagesItem {
        pub code: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub documentation_url: ::std::option::Option<::std::string::String>,
        pub message: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<DnsRecordsMessagesItemSource>,
    }

    ///`DnsRecordsMessagesItemSource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "pointer": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsMessagesItemSource {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pointer: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for DnsRecordsMessagesItemSource {
        fn default() -> Self {
            Self {
                pointer: Default::default(),
            }
        }
    }

    ///DNS record type (simplified). Original: dns-records_MXRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_MXRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsMxRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsMxRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsMxRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsMxRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsMxRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///Complete DNS record name, including the zone name, in Punycode.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Complete DNS record name, including the zone name, in
    /// Punycode.",
    ///  "examples": [
    ///    "example.com"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 255,
    ///  "minLength": 1,
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct DnsRecordsName(::std::string::String);
    impl ::std::ops::Deref for DnsRecordsName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsName> for ::std::string::String {
        fn from(value: DnsRecordsName) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for DnsRecordsName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 255usize {
                return Err("longer than 255 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DnsRecordsName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DnsRecordsName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for DnsRecordsName {
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

    ///DNS record type (simplified). Original: dns-records_NAPTRRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_NAPTRRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsNaptrRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsNaptrRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsNaptrRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsNaptrRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsNaptrRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///DNS record type (simplified). Original: dns-records_NSRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_NSRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsNsRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsNsRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsNsRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsNsRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsNsRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///DNS record type (simplified). Original: dns-records_OPENPGPKEYRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_OPENPGPKEYRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsOpenpgpkeyRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsOpenpgpkeyRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsOpenpgpkeyRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsOpenpgpkeyRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsOpenpgpkeyRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///Field to order DNS records by.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Field to order DNS records by.",
    ///  "default": "type",
    ///  "type": "string",
    ///  "enum": [
    ///    "type",
    ///    "name",
    ///    "content",
    ///    "ttl",
    ///    "proxied"
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
    pub enum DnsRecordsOrder {
        #[serde(rename = "type")]
        Type,
        #[serde(rename = "name")]
        Name,
        #[serde(rename = "content")]
        Content,
        #[serde(rename = "ttl")]
        Ttl,
        #[serde(rename = "proxied")]
        Proxied,
    }

    impl ::std::fmt::Display for DnsRecordsOrder {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Type => f.write_str("type"),
                Self::Name => f.write_str("name"),
                Self::Content => f.write_str("content"),
                Self::Ttl => f.write_str("ttl"),
                Self::Proxied => f.write_str("proxied"),
            }
        }
    }

    impl ::std::str::FromStr for DnsRecordsOrder {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "type" => Ok(Self::Type),
                "name" => Ok(Self::Name),
                "content" => Ok(Self::Content),
                "ttl" => Ok(Self::Ttl),
                "proxied" => Ok(Self::Proxied),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsOrder {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DnsRecordsOrder {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DnsRecordsOrder {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for DnsRecordsOrder {
        fn default() -> Self {
            DnsRecordsOrder::Type
        }
    }

    ///Page number of paginated results.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Page number of paginated results.",
    ///  "default": 1,
    ///  "type": "number",
    ///  "minimum": 1.0
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsPage(pub f64);
    impl ::std::ops::Deref for DnsRecordsPage {
        type Target = f64;
        fn deref(&self) -> &f64 {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsPage> for f64 {
        fn from(value: DnsRecordsPage) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<f64> for DnsRecordsPage {
        fn from(value: f64) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DnsRecordsPage {
        type Err = <f64 as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsPage {
        type Error = <f64 as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for DnsRecordsPage {
        type Error = <f64 as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for DnsRecordsPage {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Number of DNS records per page.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Number of DNS records per page.",
    ///  "default": 100,
    ///  "examples": [
    ///    5
    ///  ],
    ///  "type": "number",
    ///  "maximum": 5000000.0,
    ///  "minimum": 1.0
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsPerPage(pub f64);
    impl ::std::ops::Deref for DnsRecordsPerPage {
        type Target = f64;
        fn deref(&self) -> &f64 {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsPerPage> for f64 {
        fn from(value: DnsRecordsPerPage) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<f64> for DnsRecordsPerPage {
        fn from(value: f64) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DnsRecordsPerPage {
        type Err = <f64 as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsPerPage {
        type Error = <f64 as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for DnsRecordsPerPage {
        type Error = <f64 as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for DnsRecordsPerPage {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Required for MX and URI records; ignored for other record types (but may
    /// still be returned by the API). Records with lower priorities are
    /// preferred. This field is to be deprecated in favor of the priority field
    /// within the data map.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Required for MX and URI records; ignored for other
    /// record types (but may still be returned by the API). Records with lower
    /// priorities are preferred. This field is to be deprecated in favor of the
    /// priority field within the data map.",
    ///  "examples": [
    ///    10
    ///  ],
    ///  "type": "number",
    ///  "maximum": 65535.0,
    ///  "minimum": 0.0,
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsPriority(pub f64);
    impl ::std::ops::Deref for DnsRecordsPriority {
        type Target = f64;
        fn deref(&self) -> &f64 {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsPriority> for f64 {
        fn from(value: DnsRecordsPriority) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<f64> for DnsRecordsPriority {
        fn from(value: f64) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DnsRecordsPriority {
        type Err = <f64 as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsPriority {
        type Error = <f64 as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for DnsRecordsPriority {
        type Error = <f64 as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for DnsRecordsPriority {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Whether the record is receiving the performance and security benefits of
    /// Cloudflare.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Whether the record is receiving the performance and
    /// security benefits of Cloudflare.",
    ///  "default": false,
    ///  "examples": [
    ///    true
    ///  ],
    ///  "type": "boolean",
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsProxied(pub bool);
    impl ::std::ops::Deref for DnsRecordsProxied {
        type Target = bool;
        fn deref(&self) -> &bool {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsProxied> for bool {
        fn from(value: DnsRecordsProxied) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<bool> for DnsRecordsProxied {
        fn from(value: bool) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DnsRecordsProxied {
        type Err = <bool as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsProxied {
        type Error = <bool as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for DnsRecordsProxied {
        type Error = <bool as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for DnsRecordsProxied {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///DNS record type (simplified). Original: dns-records_PTRRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_PTRRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsPtrRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsPtrRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsPtrRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsPtrRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsPtrRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///Allows searching in multiple properties of a DNS record simultaneously.
    /// This parameter is intended for human users, not automation. Its exact
    /// behavior is intentionally left unspecified and is subject to change in
    /// the future. This parameter works independently of the `match` setting.
    /// For automated searches, please use the other available parameters.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Allows searching in multiple properties of a DNS record
    /// simultaneously. This parameter is intended for human users, not
    /// automation. Its exact behavior is intentionally left unspecified and is
    /// subject to change in the future. This parameter works independently of
    /// the `match` setting. For automated searches, please use the other
    /// available parameters.\n",
    ///  "examples": [
    ///    "www.cloudflare.com"
    ///  ],
    ///  "type": "string"
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    #[serde(transparent)]
    pub struct DnsRecordsSearch(pub ::std::string::String);
    impl ::std::ops::Deref for DnsRecordsSearch {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsSearch> for ::std::string::String {
        fn from(value: DnsRecordsSearch) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::string::String> for DnsRecordsSearch {
        fn from(value: ::std::string::String) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DnsRecordsSearch {
        type Err = ::std::convert::Infallible;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::fmt::Display for DnsRecordsSearch {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Settings for the DNS record.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Settings for the DNS record.",
    ///  "type": "object",
    ///  "properties": {
    ///    "ipv4_only": {
    ///      "description": "When enabled, only A records will be generated, and
    /// AAAA records will not be created. This setting is intended for
    /// exceptional cases. Note that this option only applies to proxied records
    /// and it has no effect on whether Cloudflare communicates with the origin
    /// using IPv4 or IPv6.",
    ///      "default": false,
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "ipv6_only": {
    ///      "description": "When enabled, only AAAA records will be generated,
    /// and A records will not be created. This setting is intended for
    /// exceptional cases. Note that this option only applies to proxied records
    /// and it has no effect on whether Cloudflare communicates with the origin
    /// using IPv4 or IPv6.",
    ///      "default": false,
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnsRecordsSettings {
        ///When enabled, only A records will be generated, and AAAA records
        /// will not be created. This setting is intended for exceptional cases.
        /// Note that this option only applies to proxied records and it has no
        /// effect on whether Cloudflare communicates with the origin using IPv4
        /// or IPv6.
        #[serde(default)]
        pub ipv4_only: bool,
        ///When enabled, only AAAA records will be generated, and A records
        /// will not be created. This setting is intended for exceptional cases.
        /// Note that this option only applies to proxied records and it has no
        /// effect on whether Cloudflare communicates with the origin using IPv4
        /// or IPv6.
        #[serde(default)]
        pub ipv6_only: bool,
    }

    impl ::std::default::Default for DnsRecordsSettings {
        fn default() -> Self {
            Self {
                ipv4_only: Default::default(),
                ipv6_only: Default::default(),
            }
        }
    }

    ///DNS record type (simplified). Original: dns-records_SMIMEARecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_SMIMEARecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsSmimeaRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsSmimeaRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsSmimeaRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsSmimeaRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsSmimeaRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///DNS record type (simplified). Original: dns-records_SRVRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_SRVRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsSrvRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsSrvRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsSrvRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsSrvRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsSrvRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///DNS record type (simplified). Original: dns-records_SSHFPRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_SSHFPRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsSshfpRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsSshfpRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsSshfpRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsSshfpRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsSshfpRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///DNS record type (simplified). Original: dns-records_SVCBRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_SVCBRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsSvcbRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsSvcbRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsSvcbRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsSvcbRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsSvcbRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///Whether to match all tag search requirements or at least one (any). If
    /// set to `all`, acts like a logical AND between tag filters. If set to
    /// `any`, acts like a logical OR instead. Note that the regular `match`
    /// parameter is still used to combine the resulting condition with other
    /// filters that aren't related to tags.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Whether to match all tag search requirements or at
    /// least one (any). If set to `all`, acts like a logical AND between tag
    /// filters. If set to `any`, acts like a logical OR instead. Note that the
    /// regular `match` parameter is still used to combine the resulting
    /// condition with other filters that aren't related to tags.\n",
    ///  "default": "all",
    ///  "examples": [
    ///    "any"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "any",
    ///    "all"
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
    pub enum DnsRecordsTagMatch {
        #[serde(rename = "any")]
        Any,
        #[serde(rename = "all")]
        All,
    }

    impl ::std::fmt::Display for DnsRecordsTagMatch {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Any => f.write_str("any"),
                Self::All => f.write_str("all"),
            }
        }
    }

    impl ::std::str::FromStr for DnsRecordsTagMatch {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "any" => Ok(Self::Any),
                "all" => Ok(Self::All),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsTagMatch {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DnsRecordsTagMatch {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DnsRecordsTagMatch {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for DnsRecordsTagMatch {
        fn default() -> Self {
            DnsRecordsTagMatch::All
        }
    }

    ///Custom tags for the DNS record. This field has no effect on DNS
    /// responses.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Custom tags for the DNS record. This field has no
    /// effect on DNS responses.",
    ///  "default": [],
    ///  "type": "array",
    ///  "items": {
    ///    "description": "Individual tag of the form name:value (the name must
    /// consist of only letters, numbers, underscores and hyphens)",
    ///    "examples": [
    ///      "owner:dns-team"
    ///    ],
    ///    "type": "string",
    ///    "x-auditable": true
    ///  },
    ///  "x-stainless-collection-type": "set"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsTags(pub ::std::vec::Vec<::std::string::String>);
    impl ::std::ops::Deref for DnsRecordsTags {
        type Target = ::std::vec::Vec<::std::string::String>;
        fn deref(&self) -> &::std::vec::Vec<::std::string::String> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsTags> for ::std::vec::Vec<::std::string::String> {
        fn from(value: DnsRecordsTags) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::vec::Vec<::std::string::String>> for DnsRecordsTags {
        fn from(value: ::std::vec::Vec<::std::string::String>) -> Self {
            Self(value)
        }
    }

    ///DNS record type (simplified). Original: dns-records_TLSARecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_TLSARecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsTlsaRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsTlsaRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsTlsaRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsTlsaRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsTlsaRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///TTL in seconds. 1 means automatic.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "TTL in seconds. 1 means automatic.",
    ///  "default": 1,
    ///  "examples": [
    ///    3600
    ///  ],
    ///  "type": "number"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsTtl(pub f64);
    impl ::std::ops::Deref for DnsRecordsTtl {
        type Target = f64;
        fn deref(&self) -> &f64 {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsTtl> for f64 {
        fn from(value: DnsRecordsTtl) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<f64> for DnsRecordsTtl {
        fn from(value: f64) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DnsRecordsTtl {
        type Err = <f64 as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsTtl {
        type Error = <f64 as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for DnsRecordsTtl {
        type Error = <f64 as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for DnsRecordsTtl {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///DNS record type (simplified). Original: dns-records_TXTRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_TXTRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsTxtRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsTxtRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsTxtRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsTxtRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsTxtRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///Record type.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Record type.",
    ///  "examples": [
    ///    "A"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "A",
    ///    "AAAA",
    ///    "CAA",
    ///    "CERT",
    ///    "CNAME",
    ///    "DNSKEY",
    ///    "DS",
    ///    "HTTPS",
    ///    "LOC",
    ///    "MX",
    ///    "NAPTR",
    ///    "NS",
    ///    "OPENPGPKEY",
    ///    "PTR",
    ///    "SMIMEA",
    ///    "SRV",
    ///    "SSHFP",
    ///    "SVCB",
    ///    "TLSA",
    ///    "TXT",
    ///    "URI"
    ///  ],
    ///  "x-auditable": true
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
    pub enum DnsRecordsType {
        A,
        #[serde(rename = "AAAA")]
        Aaaa,
        #[serde(rename = "CAA")]
        Caa,
        #[serde(rename = "CERT")]
        Cert,
        #[serde(rename = "CNAME")]
        Cname,
        #[serde(rename = "DNSKEY")]
        Dnskey,
        #[serde(rename = "DS")]
        Ds,
        #[serde(rename = "HTTPS")]
        Https,
        #[serde(rename = "LOC")]
        Loc,
        #[serde(rename = "MX")]
        Mx,
        #[serde(rename = "NAPTR")]
        Naptr,
        #[serde(rename = "NS")]
        Ns,
        #[serde(rename = "OPENPGPKEY")]
        Openpgpkey,
        #[serde(rename = "PTR")]
        Ptr,
        #[serde(rename = "SMIMEA")]
        Smimea,
        #[serde(rename = "SRV")]
        Srv,
        #[serde(rename = "SSHFP")]
        Sshfp,
        #[serde(rename = "SVCB")]
        Svcb,
        #[serde(rename = "TLSA")]
        Tlsa,
        #[serde(rename = "TXT")]
        Txt,
        #[serde(rename = "URI")]
        Uri,
    }

    impl ::std::fmt::Display for DnsRecordsType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::A => f.write_str("A"),
                Self::Aaaa => f.write_str("AAAA"),
                Self::Caa => f.write_str("CAA"),
                Self::Cert => f.write_str("CERT"),
                Self::Cname => f.write_str("CNAME"),
                Self::Dnskey => f.write_str("DNSKEY"),
                Self::Ds => f.write_str("DS"),
                Self::Https => f.write_str("HTTPS"),
                Self::Loc => f.write_str("LOC"),
                Self::Mx => f.write_str("MX"),
                Self::Naptr => f.write_str("NAPTR"),
                Self::Ns => f.write_str("NS"),
                Self::Openpgpkey => f.write_str("OPENPGPKEY"),
                Self::Ptr => f.write_str("PTR"),
                Self::Smimea => f.write_str("SMIMEA"),
                Self::Srv => f.write_str("SRV"),
                Self::Sshfp => f.write_str("SSHFP"),
                Self::Svcb => f.write_str("SVCB"),
                Self::Tlsa => f.write_str("TLSA"),
                Self::Txt => f.write_str("TXT"),
                Self::Uri => f.write_str("URI"),
            }
        }
    }

    impl ::std::str::FromStr for DnsRecordsType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "A" => Ok(Self::A),
                "AAAA" => Ok(Self::Aaaa),
                "CAA" => Ok(Self::Caa),
                "CERT" => Ok(Self::Cert),
                "CNAME" => Ok(Self::Cname),
                "DNSKEY" => Ok(Self::Dnskey),
                "DS" => Ok(Self::Ds),
                "HTTPS" => Ok(Self::Https),
                "LOC" => Ok(Self::Loc),
                "MX" => Ok(Self::Mx),
                "NAPTR" => Ok(Self::Naptr),
                "NS" => Ok(Self::Ns),
                "OPENPGPKEY" => Ok(Self::Openpgpkey),
                "PTR" => Ok(Self::Ptr),
                "SMIMEA" => Ok(Self::Smimea),
                "SRV" => Ok(Self::Srv),
                "SSHFP" => Ok(Self::Sshfp),
                "SVCB" => Ok(Self::Svcb),
                "TLSA" => Ok(Self::Tlsa),
                "TXT" => Ok(Self::Txt),
                "URI" => Ok(Self::Uri),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DnsRecordsType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DnsRecordsType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DnsRecordsType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///DNS record type (simplified). Original: dns-records_URIRecord
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS record type (simplified). Original:
    /// dns-records_URIRecord",
    ///  "type": "object",
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnsRecordsUriRecord(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for DnsRecordsUriRecord {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<DnsRecordsUriRecord>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: DnsRecordsUriRecord) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for DnsRecordsUriRecord
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///Algorithm key code.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Algorithm key code.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "13"
    ///  ],
    ///  "type": [
    ///    "string",
    ///    "null"
    ///  ],
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecAlgorithm(pub ::std::option::Option<::std::string::String>);
    impl ::std::ops::Deref for DnssecAlgorithm {
        type Target = ::std::option::Option<::std::string::String>;
        fn deref(&self) -> &::std::option::Option<::std::string::String> {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecAlgorithm> for ::std::option::Option<::std::string::String> {
        fn from(value: DnssecAlgorithm) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<::std::string::String>> for DnssecAlgorithm {
        fn from(value: ::std::option::Option<::std::string::String>) -> Self {
            Self(value)
        }
    }

    ///`DnssecApiResponseCommon`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "$ref": "#/components/schemas/dnssec_messages"
    ///    },
    ///    "messages": {
    ///      "$ref": "#/components/schemas/dnssec_messages"
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "enum": [
    ///        true
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnssecApiResponseCommon {
        pub errors: DnssecMessages,
        pub messages: DnssecMessages,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`DnssecApiResponseCommonFailure`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "result",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "examples": [
    ///        [
    ///          {
    ///            "code": 7003,
    ///            "message": "No route for the URI"
    ///          }
    ///        ]
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/dnssec_messages"
    ///        }
    ///      ],
    ///      "minLength": 1
    ///    },
    ///    "messages": {
    ///      "examples": [
    ///        []
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/dnssec_messages"
    ///        }
    ///      ]
    ///    },
    ///    "result": {
    ///      "type": [
    ///        "object",
    ///        "null"
    ///      ],
    ///      "enum": [
    ///        null
    ///      ]
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean",
    ///      "enum": [
    ///        false
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnssecApiResponseCommonFailure {
        pub errors: ::std::vec::Vec<DnssecApiResponseCommonFailureErrorsItem>,
        pub messages: DnssecMessages,
        pub result: (),
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`DnssecApiResponseCommonFailureErrorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "documentation_url": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "source": {
    ///      "type": "object",
    ///      "properties": {
    ///        "pointer": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnssecApiResponseCommonFailureErrorsItem {
        pub code: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub documentation_url: ::std::option::Option<::std::string::String>,
        pub message: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<DnssecApiResponseCommonFailureErrorsItemSource>,
    }

    ///`DnssecApiResponseCommonFailureErrorsItemSource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "pointer": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnssecApiResponseCommonFailureErrorsItemSource {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pointer: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for DnssecApiResponseCommonFailureErrorsItemSource {
        fn default() -> Self {
            Self {
                pointer: Default::default(),
            }
        }
    }

    ///`DnssecApiResponseSingle`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dnssec_api-response-common"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecApiResponseSingle(pub DnssecApiResponseCommon);
    impl ::std::ops::Deref for DnssecApiResponseSingle {
        type Target = DnssecApiResponseCommon;
        fn deref(&self) -> &DnssecApiResponseCommon {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecApiResponseSingle> for DnssecApiResponseCommon {
        fn from(value: DnssecApiResponseSingle) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<DnssecApiResponseCommon> for DnssecApiResponseSingle {
        fn from(value: DnssecApiResponseCommon) -> Self {
            Self(value)
        }
    }

    ///`DnssecDeleteDnssecRecordsResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dnssec_delete_dnssec_response_single"
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/dnssec_api-response-common-failure"
    ///    }
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
    #[serde(deny_unknown_fields)]
    pub enum DnssecDeleteDnssecRecordsResponse {}
    ///`DnssecDeleteDnssecResponseSingle`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dnssec_api-response-single"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "examples": [
    ///            ""
    ///          ],
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnssecDeleteDnssecResponseSingle {
        pub errors: DnssecMessages,
        pub messages: DnssecMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result: ::std::option::Option<::std::string::String>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///Digest hash.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Digest hash.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "48E939042E82C22542CB377B580DFDC52A361CEFDC72E7F9107E2B6BD9306A45"
    ///  ],
    ///  "type": [
    ///    "string",
    ///    "null"
    ///  ],
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecDigest(pub ::std::option::Option<::std::string::String>);
    impl ::std::ops::Deref for DnssecDigest {
        type Target = ::std::option::Option<::std::string::String>;
        fn deref(&self) -> &::std::option::Option<::std::string::String> {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecDigest> for ::std::option::Option<::std::string::String> {
        fn from(value: DnssecDigest) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<::std::string::String>> for DnssecDigest {
        fn from(value: ::std::option::Option<::std::string::String>) -> Self {
            Self(value)
        }
    }

    ///Type of digest algorithm.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Type of digest algorithm.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "SHA256"
    ///  ],
    ///  "type": [
    ///    "string",
    ///    "null"
    ///  ],
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecDigestAlgorithm(pub ::std::option::Option<::std::string::String>);
    impl ::std::ops::Deref for DnssecDigestAlgorithm {
        type Target = ::std::option::Option<::std::string::String>;
        fn deref(&self) -> &::std::option::Option<::std::string::String> {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecDigestAlgorithm> for ::std::option::Option<::std::string::String> {
        fn from(value: DnssecDigestAlgorithm) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<::std::string::String>> for DnssecDigestAlgorithm {
        fn from(value: ::std::option::Option<::std::string::String>) -> Self {
            Self(value)
        }
    }

    ///Coded type for digest algorithm.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Coded type for digest algorithm.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "2"
    ///  ],
    ///  "type": [
    ///    "string",
    ///    "null"
    ///  ],
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecDigestType(pub ::std::option::Option<::std::string::String>);
    impl ::std::ops::Deref for DnssecDigestType {
        type Target = ::std::option::Option<::std::string::String>;
        fn deref(&self) -> &::std::option::Option<::std::string::String> {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecDigestType> for ::std::option::Option<::std::string::String> {
        fn from(value: DnssecDigestType) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<::std::string::String>> for DnssecDigestType {
        fn from(value: ::std::option::Option<::std::string::String>) -> Self {
            Self(value)
        }
    }

    ///`DnssecDnssec`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "algorithm": {
    ///      "$ref": "#/components/schemas/dnssec_algorithm"
    ///    },
    ///    "digest": {
    ///      "$ref": "#/components/schemas/dnssec_digest"
    ///    },
    ///    "digest_algorithm": {
    ///      "$ref": "#/components/schemas/dnssec_digest_algorithm"
    ///    },
    ///    "digest_type": {
    ///      "$ref": "#/components/schemas/dnssec_digest_type"
    ///    },
    ///    "dnssec_multi_signer": {
    ///      "$ref": "#/components/schemas/dnssec_dnssec_multi_signer"
    ///    },
    ///    "dnssec_presigned": {
    ///      "$ref": "#/components/schemas/dnssec_dnssec_presigned"
    ///    },
    ///    "dnssec_use_nsec3": {
    ///      "$ref": "#/components/schemas/dnssec_dnssec_use_nsec3"
    ///    },
    ///    "ds": {
    ///      "$ref": "#/components/schemas/dnssec_ds"
    ///    },
    ///    "flags": {
    ///      "$ref": "#/components/schemas/dnssec_flags"
    ///    },
    ///    "key_tag": {
    ///      "$ref": "#/components/schemas/dnssec_key_tag"
    ///    },
    ///    "key_type": {
    ///      "$ref": "#/components/schemas/dnssec_key_type"
    ///    },
    ///    "modified_on": {
    ///      "$ref": "#/components/schemas/dnssec_modified_on"
    ///    },
    ///    "public_key": {
    ///      "$ref": "#/components/schemas/dnssec_public_key"
    ///    },
    ///    "status": {
    ///      "$ref": "#/components/schemas/dnssec_status"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnssecDnssec {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub algorithm: ::std::option::Option<DnssecAlgorithm>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub digest: ::std::option::Option<DnssecDigest>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub digest_algorithm: ::std::option::Option<DnssecDigestAlgorithm>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub digest_type: ::std::option::Option<DnssecDigestType>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub dnssec_multi_signer: ::std::option::Option<DnssecDnssecMultiSigner>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub dnssec_presigned: ::std::option::Option<DnssecDnssecPresigned>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub dnssec_use_nsec3: ::std::option::Option<DnssecDnssecUseNsec3>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ds: ::std::option::Option<DnssecDs>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub flags: ::std::option::Option<DnssecFlags>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub key_tag: ::std::option::Option<DnssecKeyTag>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub key_type: ::std::option::Option<DnssecKeyType>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub modified_on: ::std::option::Option<DnssecModifiedOn>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub public_key: ::std::option::Option<DnssecPublicKey>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub status: ::std::option::Option<DnssecStatus>,
    }

    impl ::std::default::Default for DnssecDnssec {
        fn default() -> Self {
            Self {
                algorithm: Default::default(),
                digest: Default::default(),
                digest_algorithm: Default::default(),
                digest_type: Default::default(),
                dnssec_multi_signer: Default::default(),
                dnssec_presigned: Default::default(),
                dnssec_use_nsec3: Default::default(),
                ds: Default::default(),
                flags: Default::default(),
                key_tag: Default::default(),
                key_type: Default::default(),
                modified_on: Default::default(),
                public_key: Default::default(),
                status: Default::default(),
            }
        }
    }

    ///`DnssecDnssecDetailsResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dnssec_dnssec_response_single"
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/dnssec_api-response-common-failure"
    ///    }
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
    #[serde(deny_unknown_fields)]
    pub enum DnssecDnssecDetailsResponse {}
    ///If true, multi-signer DNSSEC is enabled on the zone, allowing multiple
    ///providers to serve a DNSSEC-signed zone at the same time.
    ///This is required for DNSKEY records (except those automatically
    ///generated by Cloudflare) to be added to the zone.
    ///
    ///See [Multi-signer DNSSEC](https://developers.cloudflare.com/dns/dnssec/multi-signer-dnssec/) for details.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "If true, multi-signer DNSSEC is enabled on the zone, allowing multiple\nproviders to serve a DNSSEC-signed zone at the same time.\nThis is required for DNSKEY records (except those automatically\ngenerated by Cloudflare) to be added to the zone.\n\nSee [Multi-signer DNSSEC](https://developers.cloudflare.com/dns/dnssec/multi-signer-dnssec/) for details.",
    ///  "examples": [
    ///    false
    ///  ],
    ///  "type": "boolean",
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecDnssecMultiSigner(pub bool);
    impl ::std::ops::Deref for DnssecDnssecMultiSigner {
        type Target = bool;
        fn deref(&self) -> &bool {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecDnssecMultiSigner> for bool {
        fn from(value: DnssecDnssecMultiSigner) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<bool> for DnssecDnssecMultiSigner {
        fn from(value: bool) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DnssecDnssecMultiSigner {
        type Err = <bool as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for DnssecDnssecMultiSigner {
        type Error = <bool as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for DnssecDnssecMultiSigner {
        type Error = <bool as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for DnssecDnssecMultiSigner {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///If true, allows Cloudflare to transfer in a DNSSEC-signed zone
    ///including signatures from an external provider, without requiring
    ///Cloudflare to sign any records on the fly.
    ///
    ///Note that this feature has some limitations.
    ///See [Cloudflare as Secondary](https://developers.cloudflare.com/dns/zone-setups/zone-transfers/cloudflare-as-secondary/setup/#dnssec) for details.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "If true, allows Cloudflare to transfer in a DNSSEC-signed zone\nincluding signatures from an external provider, without requiring\nCloudflare to sign any records on the fly.\n\nNote that this feature has some limitations.\nSee [Cloudflare as Secondary](https://developers.cloudflare.com/dns/zone-setups/zone-transfers/cloudflare-as-secondary/setup/#dnssec) for details.",
    ///  "examples": [
    ///    true
    ///  ],
    ///  "type": "boolean",
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecDnssecPresigned(pub bool);
    impl ::std::ops::Deref for DnssecDnssecPresigned {
        type Target = bool;
        fn deref(&self) -> &bool {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecDnssecPresigned> for bool {
        fn from(value: DnssecDnssecPresigned) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<bool> for DnssecDnssecPresigned {
        fn from(value: bool) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DnssecDnssecPresigned {
        type Err = <bool as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for DnssecDnssecPresigned {
        type Error = <bool as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for DnssecDnssecPresigned {
        type Error = <bool as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for DnssecDnssecPresigned {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///`DnssecDnssecResponseSingle`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dnssec_api-response-single"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/dnssec_dnssec"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnssecDnssecResponseSingle {
        pub errors: DnssecMessages,
        pub messages: DnssecMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result: ::std::option::Option<DnssecDnssec>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///If true, enables the use of NSEC3 together with DNSSEC on the zone.
    ///Combined with setting dnssec_presigned to true, this enables the use of
    ///NSEC3 records when transferring in from an external provider.
    ///If dnssec_presigned is instead set to false (default), NSEC3 records
    /// will be generated and signed at request time.
    ///
    ///See [DNSSEC with NSEC3](https://developers.cloudflare.com/dns/dnssec/enable-nsec3/) for details.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "If true, enables the use of NSEC3 together with DNSSEC on the zone.\nCombined with setting dnssec_presigned to true, this enables the use of\nNSEC3 records when transferring in from an external provider.\nIf dnssec_presigned is instead set to false (default), NSEC3 records will be\ngenerated and signed at request time.\n\nSee [DNSSEC with NSEC3](https://developers.cloudflare.com/dns/dnssec/enable-nsec3/) for details.",
    ///  "examples": [
    ///    false
    ///  ],
    ///  "type": "boolean",
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecDnssecUseNsec3(pub bool);
    impl ::std::ops::Deref for DnssecDnssecUseNsec3 {
        type Target = bool;
        fn deref(&self) -> &bool {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecDnssecUseNsec3> for bool {
        fn from(value: DnssecDnssecUseNsec3) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<bool> for DnssecDnssecUseNsec3 {
        fn from(value: bool) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DnssecDnssecUseNsec3 {
        type Err = <bool as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for DnssecDnssecUseNsec3 {
        type Error = <bool as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for DnssecDnssecUseNsec3 {
        type Error = <bool as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for DnssecDnssecUseNsec3 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Full DS record.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Full DS record.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "example.com. 3600 IN DS 16953 13 2
    /// 48E939042E82C22542CB377B580DFDC52A361CEFDC72E7F9107E2B6BD9306A45"
    ///  ],
    ///  "type": [
    ///    "string",
    ///    "null"
    ///  ],
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecDs(pub ::std::option::Option<::std::string::String>);
    impl ::std::ops::Deref for DnssecDs {
        type Target = ::std::option::Option<::std::string::String>;
        fn deref(&self) -> &::std::option::Option<::std::string::String> {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecDs> for ::std::option::Option<::std::string::String> {
        fn from(value: DnssecDs) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<::std::string::String>> for DnssecDs {
        fn from(value: ::std::option::Option<::std::string::String>) -> Self {
            Self(value)
        }
    }

    ///`DnssecEditDnssecStatusBody`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "dnssec_multi_signer": {
    ///      "$ref": "#/components/schemas/dnssec_dnssec_multi_signer"
    ///    },
    ///    "dnssec_presigned": {
    ///      "$ref": "#/components/schemas/dnssec_dnssec_presigned"
    ///    },
    ///    "dnssec_use_nsec3": {
    ///      "$ref": "#/components/schemas/dnssec_dnssec_use_nsec3"
    ///    },
    ///    "status": {
    ///      "description": "Status of DNSSEC, based on user-desired state and
    /// presence of necessary records.",
    ///      "examples": [
    ///        "active"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "active",
    ///        "disabled"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnssecEditDnssecStatusBody {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub dnssec_multi_signer: ::std::option::Option<DnssecDnssecMultiSigner>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub dnssec_presigned: ::std::option::Option<DnssecDnssecPresigned>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub dnssec_use_nsec3: ::std::option::Option<DnssecDnssecUseNsec3>,
        ///Status of DNSSEC, based on user-desired state and presence of
        /// necessary records.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub status: ::std::option::Option<DnssecEditDnssecStatusBodyStatus>,
    }

    impl ::std::default::Default for DnssecEditDnssecStatusBody {
        fn default() -> Self {
            Self {
                dnssec_multi_signer: Default::default(),
                dnssec_presigned: Default::default(),
                dnssec_use_nsec3: Default::default(),
                status: Default::default(),
            }
        }
    }

    ///Status of DNSSEC, based on user-desired state and presence of necessary
    /// records.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Status of DNSSEC, based on user-desired state and
    /// presence of necessary records.",
    ///  "examples": [
    ///    "active"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "active",
    ///    "disabled"
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
    pub enum DnssecEditDnssecStatusBodyStatus {
        #[serde(rename = "active")]
        Active,
        #[serde(rename = "disabled")]
        Disabled,
    }

    impl ::std::fmt::Display for DnssecEditDnssecStatusBodyStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Active => f.write_str("active"),
                Self::Disabled => f.write_str("disabled"),
            }
        }
    }

    impl ::std::str::FromStr for DnssecEditDnssecStatusBodyStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "active" => Ok(Self::Active),
                "disabled" => Ok(Self::Disabled),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DnssecEditDnssecStatusBodyStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DnssecEditDnssecStatusBodyStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DnssecEditDnssecStatusBodyStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`DnssecEditDnssecStatusResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/dnssec_dnssec_response_single"
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/dnssec_api-response-common-failure"
    ///    }
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
    #[serde(deny_unknown_fields)]
    pub enum DnssecEditDnssecStatusResponse {}
    ///Flag for DNSSEC record.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Flag for DNSSEC record.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    257
    ///  ],
    ///  "type": [
    ///    "number",
    ///    "null"
    ///  ],
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecFlags(pub ::std::option::Option<f64>);
    impl ::std::ops::Deref for DnssecFlags {
        type Target = ::std::option::Option<f64>;
        fn deref(&self) -> &::std::option::Option<f64> {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecFlags> for ::std::option::Option<f64> {
        fn from(value: DnssecFlags) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<f64>> for DnssecFlags {
        fn from(value: ::std::option::Option<f64>) -> Self {
            Self(value)
        }
    }

    ///Identifier.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Identifier.",
    ///  "examples": [
    ///    "023e105f4ecef8ad9ca31a8372d0c353"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32,
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct DnssecIdentifier(::std::string::String);
    impl ::std::ops::Deref for DnssecIdentifier {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecIdentifier> for ::std::string::String {
        fn from(value: DnssecIdentifier) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for DnssecIdentifier {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for DnssecIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DnssecIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DnssecIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for DnssecIdentifier {
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

    ///Code for key tag.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Code for key tag.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    42
    ///  ],
    ///  "type": [
    ///    "number",
    ///    "null"
    ///  ],
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecKeyTag(pub ::std::option::Option<f64>);
    impl ::std::ops::Deref for DnssecKeyTag {
        type Target = ::std::option::Option<f64>;
        fn deref(&self) -> &::std::option::Option<f64> {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecKeyTag> for ::std::option::Option<f64> {
        fn from(value: DnssecKeyTag) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<f64>> for DnssecKeyTag {
        fn from(value: ::std::option::Option<f64>) -> Self {
            Self(value)
        }
    }

    ///Algorithm key type.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Algorithm key type.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "ECDSAP256SHA256"
    ///  ],
    ///  "type": [
    ///    "string",
    ///    "null"
    ///  ],
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecKeyType(pub ::std::option::Option<::std::string::String>);
    impl ::std::ops::Deref for DnssecKeyType {
        type Target = ::std::option::Option<::std::string::String>;
        fn deref(&self) -> &::std::option::Option<::std::string::String> {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecKeyType> for ::std::option::Option<::std::string::String> {
        fn from(value: DnssecKeyType) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<::std::string::String>> for DnssecKeyType {
        fn from(value: ::std::option::Option<::std::string::String>) -> Self {
            Self(value)
        }
    }

    ///`DnssecMessages`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    []
    ///  ],
    ///  "type": "array",
    ///  "items": {
    ///    "type": "object",
    ///    "uniqueItems": true,
    ///    "required": [
    ///      "code",
    ///      "message"
    ///    ],
    ///    "properties": {
    ///      "code": {
    ///        "type": "integer",
    ///        "minimum": 1000.0
    ///      },
    ///      "documentation_url": {
    ///        "type": "string"
    ///      },
    ///      "message": {
    ///        "type": "string"
    ///      },
    ///      "source": {
    ///        "type": "object",
    ///        "properties": {
    ///          "pointer": {
    ///            "type": "string"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecMessages(pub ::std::vec::Vec<DnssecMessagesItem>);
    impl ::std::ops::Deref for DnssecMessages {
        type Target = ::std::vec::Vec<DnssecMessagesItem>;
        fn deref(&self) -> &::std::vec::Vec<DnssecMessagesItem> {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecMessages> for ::std::vec::Vec<DnssecMessagesItem> {
        fn from(value: DnssecMessages) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::vec::Vec<DnssecMessagesItem>> for DnssecMessages {
        fn from(value: ::std::vec::Vec<DnssecMessagesItem>) -> Self {
            Self(value)
        }
    }

    ///`DnssecMessagesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "documentation_url": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "source": {
    ///      "type": "object",
    ///      "properties": {
    ///        "pointer": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnssecMessagesItem {
        pub code: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub documentation_url: ::std::option::Option<::std::string::String>,
        pub message: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<DnssecMessagesItemSource>,
    }

    ///`DnssecMessagesItemSource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "pointer": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DnssecMessagesItemSource {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pointer: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for DnssecMessagesItemSource {
        fn default() -> Self {
            Self {
                pointer: Default::default(),
            }
        }
    }

    ///When DNSSEC was last modified.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "When DNSSEC was last modified.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "2014-01-01T05:20:00Z"
    ///  ],
    ///  "type": [
    ///    "string",
    ///    "null"
    ///  ],
    ///  "format": "date-time",
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecModifiedOn(
        pub ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
    );
    impl ::std::ops::Deref for DnssecModifiedOn {
        type Target = ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>;
        fn deref(&self) -> &::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>> {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecModifiedOn>
        for ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>
    {
        fn from(value: DnssecModifiedOn) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>>
        for DnssecModifiedOn
    {
        fn from(value: ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>) -> Self {
            Self(value)
        }
    }

    ///Public key for DS record.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Public key for DS record.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "oXiGYrSTO+LSCJ3mohc8EP+CzF9KxBj8/ydXJ22pKuZP3VAC3/Md/
    /// k7xZfz470CoRyZJ6gV6vml07IC3d8xqhA=="
    ///  ],
    ///  "type": [
    ///    "string",
    ///    "null"
    ///  ],
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct DnssecPublicKey(pub ::std::option::Option<::std::string::String>);
    impl ::std::ops::Deref for DnssecPublicKey {
        type Target = ::std::option::Option<::std::string::String>;
        fn deref(&self) -> &::std::option::Option<::std::string::String> {
            &self.0
        }
    }

    impl ::std::convert::From<DnssecPublicKey> for ::std::option::Option<::std::string::String> {
        fn from(value: DnssecPublicKey) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<::std::string::String>> for DnssecPublicKey {
        fn from(value: ::std::option::Option<::std::string::String>) -> Self {
            Self(value)
        }
    }

    ///Status of DNSSEC, based on user-desired state and presence of necessary
    /// records.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Status of DNSSEC, based on user-desired state and
    /// presence of necessary records.",
    ///  "examples": [
    ///    "active"
    ///  ],
    ///  "enum": [
    ///    "active",
    ///    "pending",
    ///    "disabled",
    ///    "pending-disabled",
    ///    "error"
    ///  ],
    ///  "x-auditable": true
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
    pub enum DnssecStatus {
        #[serde(rename = "active")]
        Active,
        #[serde(rename = "pending")]
        Pending,
        #[serde(rename = "disabled")]
        Disabled,
        #[serde(rename = "pending-disabled")]
        PendingDisabled,
        #[serde(rename = "error")]
        Error,
    }

    impl ::std::fmt::Display for DnssecStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Active => f.write_str("active"),
                Self::Pending => f.write_str("pending"),
                Self::Disabled => f.write_str("disabled"),
                Self::PendingDisabled => f.write_str("pending-disabled"),
                Self::Error => f.write_str("error"),
            }
        }
    }

    impl ::std::str::FromStr for DnssecStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "active" => Ok(Self::Active),
                "pending" => Ok(Self::Pending),
                "disabled" => Ok(Self::Disabled),
                "pending-disabled" => Ok(Self::PendingDisabled),
                "error" => Ok(Self::Error),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DnssecStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DnssecStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DnssecStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`IamAccount`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "id",
    ///    "name",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "created_on": {
    ///      "description": "Timestamp for the creation of the account",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "2014-03-01T12:21:02.0000Z"
    ///      ],
    ///      "type": "string",
    ///      "format": "date-time",
    ///      "x-auditable": true
    ///    },
    ///    "id": {
    ///      "$ref":
    /// "#/components/schemas/iam_common_components-schemas-identifier"
    ///    },
    ///    "managed_by": {
    ///      "description": "Parent container details",
    ///      "type": "object",
    ///      "properties": {
    ///        "parent_org_id": {
    ///          "description": "ID of the parent Organization, if one exists",
    ///          "readOnly": true,
    ///          "examples": [
    ///            "4536bcfad5faccb111b47003c79917fa"
    ///          ],
    ///          "type": "string",
    ///          "maxLength": 32,
    ///          "x-auditable": true
    ///        },
    ///        "parent_org_name": {
    ///          "description": "Name of the parent Organization, if one
    /// exists",
    ///          "readOnly": true,
    ///          "examples": [
    ///            "Demo Parent Organization"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    },
    ///    "name": {
    ///      "description": "Account name",
    ///      "examples": [
    ///        "Demo Account"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 100,
    ///      "x-auditable": true
    ///    },
    ///    "settings": {
    ///      "description": "Account settings",
    ///      "type": "object",
    ///      "properties": {
    ///        "abuse_contact_email": {
    ///          "description": "Sets an abuse contact email to notify for abuse
    /// reports.",
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "enforce_twofactor": {
    ///          "description": "Indicates whether membership in this account
    /// requires that\nTwo-Factor Authentication is enabled",
    ///          "default": false,
    ///          "type": "boolean",
    ///          "x-auditable": true,
    ///          "x-stainless-terraform-configurability": "computed_optional"
    ///        }
    ///      }
    ///    },
    ///    "type": {
    ///      "$ref": "#/components/schemas/iam_account-type"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamAccount {
        ///Timestamp for the creation of the account
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub created_on: ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
        pub id: IamCommonComponentsSchemasIdentifier,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub managed_by: ::std::option::Option<IamAccountManagedBy>,
        ///Account name
        pub name: IamAccountName,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub settings: ::std::option::Option<IamAccountSettings>,
        #[serde(rename = "type")]
        pub type_: IamAccountType,
    }

    ///Account identifier tag.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "title": "Account Identifier",
    ///  "description": "Account identifier tag.",
    ///  "allOf": [
    ///    {
    ///      "$ref":
    /// "#/components/schemas/iam_common_components-schemas-identifier"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct IamAccountIdentifier(pub IamCommonComponentsSchemasIdentifier);
    impl ::std::ops::Deref for IamAccountIdentifier {
        type Target = IamCommonComponentsSchemasIdentifier;
        fn deref(&self) -> &IamCommonComponentsSchemasIdentifier {
            &self.0
        }
    }

    impl ::std::convert::From<IamAccountIdentifier> for IamCommonComponentsSchemasIdentifier {
        fn from(value: IamAccountIdentifier) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<IamCommonComponentsSchemasIdentifier> for IamAccountIdentifier {
        fn from(value: IamCommonComponentsSchemasIdentifier) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for IamAccountIdentifier {
        type Err = <IamCommonComponentsSchemasIdentifier as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for IamAccountIdentifier {
        type Error = <IamCommonComponentsSchemasIdentifier as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for IamAccountIdentifier {
        type Error = <IamCommonComponentsSchemasIdentifier as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for IamAccountIdentifier {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Parent container details
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Parent container details",
    ///  "type": "object",
    ///  "properties": {
    ///    "parent_org_id": {
    ///      "description": "ID of the parent Organization, if one exists",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "4536bcfad5faccb111b47003c79917fa"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 32,
    ///      "x-auditable": true
    ///    },
    ///    "parent_org_name": {
    ///      "description": "Name of the parent Organization, if one exists",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "Demo Parent Organization"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamAccountManagedBy {
        ///ID of the parent Organization, if one exists
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub parent_org_id: ::std::option::Option<IamAccountManagedByParentOrgId>,
        ///Name of the parent Organization, if one exists
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub parent_org_name: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for IamAccountManagedBy {
        fn default() -> Self {
            Self {
                parent_org_id: Default::default(),
                parent_org_name: Default::default(),
            }
        }
    }

    ///ID of the parent Organization, if one exists
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "ID of the parent Organization, if one exists",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "4536bcfad5faccb111b47003c79917fa"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32,
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct IamAccountManagedByParentOrgId(::std::string::String);
    impl ::std::ops::Deref for IamAccountManagedByParentOrgId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<IamAccountManagedByParentOrgId> for ::std::string::String {
        fn from(value: IamAccountManagedByParentOrgId) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for IamAccountManagedByParentOrgId {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for IamAccountManagedByParentOrgId {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for IamAccountManagedByParentOrgId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for IamAccountManagedByParentOrgId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for IamAccountManagedByParentOrgId {
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

    ///Account name
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Account name",
    ///  "examples": [
    ///    "Demo Account"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 100,
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct IamAccountName(::std::string::String);
    impl ::std::ops::Deref for IamAccountName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<IamAccountName> for ::std::string::String {
        fn from(value: IamAccountName) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for IamAccountName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 100usize {
                return Err("longer than 100 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for IamAccountName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for IamAccountName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for IamAccountName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for IamAccountName {
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

    ///Account settings
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Account settings",
    ///  "type": "object",
    ///  "properties": {
    ///    "abuse_contact_email": {
    ///      "description": "Sets an abuse contact email to notify for abuse
    /// reports.",
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "enforce_twofactor": {
    ///      "description": "Indicates whether membership in this account
    /// requires that\nTwo-Factor Authentication is enabled",
    ///      "default": false,
    ///      "type": "boolean",
    ///      "x-auditable": true,
    ///      "x-stainless-terraform-configurability": "computed_optional"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamAccountSettings {
        ///Sets an abuse contact email to notify for abuse reports.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub abuse_contact_email: ::std::option::Option<::std::string::String>,
        ///Indicates whether membership in this account requires that
        ///Two-Factor Authentication is enabled
        #[serde(default)]
        pub enforce_twofactor: bool,
    }

    impl ::std::default::Default for IamAccountSettings {
        fn default() -> Self {
            Self {
                abuse_contact_email: Default::default(),
                enforce_twofactor: Default::default(),
            }
        }
    }

    ///`IamAccountType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "enum": [
    ///    "standard",
    ///    "enterprise"
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
    pub enum IamAccountType {
        #[serde(rename = "standard")]
        Standard,
        #[serde(rename = "enterprise")]
        Enterprise,
    }

    impl ::std::fmt::Display for IamAccountType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Standard => f.write_str("standard"),
                Self::Enterprise => f.write_str("enterprise"),
            }
        }
    }

    impl ::std::str::FromStr for IamAccountType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "standard" => Ok(Self::Standard),
                "enterprise" => Ok(Self::Enterprise),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for IamAccountType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for IamAccountType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for IamAccountType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`IamApiResponseCollection`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/iam_api-response-common"
    ///    },
    ///    {
    ///      "properties": {
    ///        "result_info": {
    ///          "$ref": "#/components/schemas/iam_result_info"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamApiResponseCollection {
        pub errors: IamSchemasMessages,
        pub messages: IamSchemasMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result_info: ::std::option::Option<IamResultInfo>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`IamApiResponseCommon`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "$ref": "#/components/schemas/iam_schemas-messages"
    ///    },
    ///    "messages": {
    ///      "$ref": "#/components/schemas/iam_schemas-messages"
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "enum": [
    ///        true
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamApiResponseCommon {
        pub errors: IamSchemasMessages,
        pub messages: IamSchemasMessages,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`IamApiResponseCommonFailure`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "result",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "examples": [
    ///        [
    ///          {
    ///            "code": 7003,
    ///            "message": "No route for the URI"
    ///          }
    ///        ]
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/iam_schemas-messages"
    ///        }
    ///      ],
    ///      "minLength": 1
    ///    },
    ///    "messages": {
    ///      "examples": [
    ///        []
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/iam_schemas-messages"
    ///        }
    ///      ]
    ///    },
    ///    "result": {
    ///      "type": [
    ///        "object",
    ///        "null"
    ///      ],
    ///      "enum": [
    ///        null
    ///      ]
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean",
    ///      "enum": [
    ///        false
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamApiResponseCommonFailure {
        pub errors: ::std::vec::Vec<IamApiResponseCommonFailureErrorsItem>,
        pub messages: IamSchemasMessages,
        pub result: (),
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`IamApiResponseCommonFailureErrorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "documentation_url": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "source": {
    ///      "type": "object",
    ///      "properties": {
    ///        "pointer": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamApiResponseCommonFailureErrorsItem {
        pub code: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub documentation_url: ::std::option::Option<::std::string::String>,
        pub message: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<IamApiResponseCommonFailureErrorsItemSource>,
    }

    ///`IamApiResponseCommonFailureErrorsItemSource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "pointer": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamApiResponseCommonFailureErrorsItemSource {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pointer: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for IamApiResponseCommonFailureErrorsItemSource {
        fn default() -> Self {
            Self {
                pointer: Default::default(),
            }
        }
    }

    ///`IamApiResponseSingle`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/iam_api-response-common"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct IamApiResponseSingle(pub IamApiResponseCommon);
    impl ::std::ops::Deref for IamApiResponseSingle {
        type Target = IamApiResponseCommon;
        fn deref(&self) -> &IamApiResponseCommon {
            &self.0
        }
    }

    impl ::std::convert::From<IamApiResponseSingle> for IamApiResponseCommon {
        fn from(value: IamApiResponseSingle) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<IamApiResponseCommon> for IamApiResponseSingle {
        fn from(value: IamApiResponseCommon) -> Self {
            Self(value)
        }
    }

    ///`IamApiResponseSingleId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/iam_api-response-common"
    ///    },
    ///    {
    ///      "properties": {
    ///        "result": {
    ///          "type": [
    ///            "object",
    ///            "null"
    ///          ],
    ///          "required": [
    ///            "id"
    ///          ],
    ///          "properties": {
    ///            "id": {
    ///              "$ref":
    /// "#/components/schemas/iam_common_components-schemas-identifier"
    ///            }
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamApiResponseSingleId {
        pub errors: IamSchemasMessages,
        pub messages: IamSchemasMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result: ::std::option::Option<IamApiResponseSingleIdResult>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`IamApiResponseSingleIdResult`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "id"
    ///  ],
    ///  "properties": {
    ///    "id": {
    ///      "$ref":
    /// "#/components/schemas/iam_common_components-schemas-identifier"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamApiResponseSingleIdResult {
        pub id: IamCommonComponentsSchemasIdentifier,
    }

    ///Identifier
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Identifier",
    ///  "examples": [
    ///    "023e105f4ecef8ad9ca31a8372d0c353"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32,
    ///  "minLength": 32,
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct IamCommonComponentsSchemasIdentifier(::std::string::String);
    impl ::std::ops::Deref for IamCommonComponentsSchemasIdentifier {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<IamCommonComponentsSchemasIdentifier> for ::std::string::String {
        fn from(value: IamCommonComponentsSchemasIdentifier) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for IamCommonComponentsSchemasIdentifier {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            if value.chars().count() < 32usize {
                return Err("shorter than 32 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for IamCommonComponentsSchemasIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for IamCommonComponentsSchemasIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for IamCommonComponentsSchemasIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for IamCommonComponentsSchemasIdentifier {
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

    ///`IamComponentsSchemasAccount`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/iam_account"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct IamComponentsSchemasAccount(pub IamAccount);
    impl ::std::ops::Deref for IamComponentsSchemasAccount {
        type Target = IamAccount;
        fn deref(&self) -> &IamAccount {
            &self.0
        }
    }

    impl ::std::convert::From<IamComponentsSchemasAccount> for IamAccount {
        fn from(value: IamComponentsSchemasAccount) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<IamAccount> for IamComponentsSchemasAccount {
        fn from(value: IamAccount) -> Self {
            Self(value)
        }
    }

    ///`IamCreateAccount`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "title": "Create account",
    ///  "type": "object",
    ///  "required": [
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "name": {
    ///      "description": "Account name",
    ///      "type": "string"
    ///    },
    ///    "type": {
    ///      "$ref": "#/components/schemas/iam_account-type"
    ///    },
    ///    "unit": {
    ///      "description": "information related to the tenant unit, and optionally, an id of the unit to create the account on. see https://developers.cloudflare.com/tenant/how-to/manage-accounts/",
    ///      "type": "object",
    ///      "properties": {
    ///        "id": {
    ///          "description": "Tenant unit ID",
    ///          "examples": [
    ///            "f267e341f3dd4697bd3b9f71dd96247f"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true,
    ///          "x-stainless-terraform-configurability": "computed_optional"
    ///        }
    ///      },
    ///      "x-stainless-terraform-configurability": "computed_optional"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamCreateAccount {
        ///Account name
        pub name: ::std::string::String,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<IamAccountType>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub unit: ::std::option::Option<IamCreateAccountUnit>,
    }

    ///information related to the tenant unit, and optionally, an id of the unit to create the account on. see https://developers.cloudflare.com/tenant/how-to/manage-accounts/
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "information related to the tenant unit, and optionally, an id of the unit to create the account on. see https://developers.cloudflare.com/tenant/how-to/manage-accounts/",
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
    ///      "description": "Tenant unit ID",
    ///      "examples": [
    ///        "f267e341f3dd4697bd3b9f71dd96247f"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true,
    ///      "x-stainless-terraform-configurability": "computed_optional"
    ///    }
    ///  },
    ///  "x-stainless-terraform-configurability": "computed_optional"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamCreateAccountUnit {
        ///Tenant unit ID
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for IamCreateAccountUnit {
        fn default() -> Self {
            Self {
                id: Default::default(),
            }
        }
    }

    ///`IamResponseCollectionAccounts`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "title": "Response with a list of accounts",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/iam_api-response-collection"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "type": "array",
    ///          "items": {
    ///            "$ref": "#/components/schemas/iam_account"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamResponseCollectionAccounts {
        pub errors: IamSchemasMessages,
        pub messages: IamSchemasMessages,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub result: ::std::vec::Vec<IamAccount>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result_info: ::std::option::Option<IamResultInfo>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`IamResponseSingleAccount`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "title": "Response with a single account",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/iam_api-response-single"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/iam_account"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamResponseSingleAccount {
        pub errors: IamSchemasMessages,
        pub messages: IamSchemasMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result: ::std::option::Option<IamAccount>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`IamResultInfo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "count": {
    ///      "description": "Total number of results for the requested service",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "page": {
    ///      "description": "Current page within paginated list of results",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "per_page": {
    ///      "description": "Number of results per page of results",
    ///      "examples": [
    ///        20
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_count": {
    ///      "description": "Total results available without any search
    /// parameters",
    ///      "examples": [
    ///        2000
    ///      ],
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamResultInfo {
        ///Total number of results for the requested service
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub count: ::std::option::Option<f64>,
        ///Current page within paginated list of results
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub page: ::std::option::Option<f64>,
        ///Number of results per page of results
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub per_page: ::std::option::Option<f64>,
        ///Total results available without any search parameters
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_count: ::std::option::Option<f64>,
    }

    impl ::std::default::Default for IamResultInfo {
        fn default() -> Self {
            Self {
                count: Default::default(),
                page: Default::default(),
                per_page: Default::default(),
                total_count: Default::default(),
            }
        }
    }

    ///`IamSchemasMessages`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    []
    ///  ],
    ///  "type": "array",
    ///  "items": {
    ///    "type": "object",
    ///    "uniqueItems": true,
    ///    "required": [
    ///      "code",
    ///      "message"
    ///    ],
    ///    "properties": {
    ///      "code": {
    ///        "type": "integer",
    ///        "minimum": 1000.0
    ///      },
    ///      "documentation_url": {
    ///        "type": "string"
    ///      },
    ///      "message": {
    ///        "type": "string"
    ///      },
    ///      "source": {
    ///        "type": "object",
    ///        "properties": {
    ///          "pointer": {
    ///            "type": "string"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct IamSchemasMessages(pub ::std::vec::Vec<IamSchemasMessagesItem>);
    impl ::std::ops::Deref for IamSchemasMessages {
        type Target = ::std::vec::Vec<IamSchemasMessagesItem>;
        fn deref(&self) -> &::std::vec::Vec<IamSchemasMessagesItem> {
            &self.0
        }
    }

    impl ::std::convert::From<IamSchemasMessages> for ::std::vec::Vec<IamSchemasMessagesItem> {
        fn from(value: IamSchemasMessages) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::vec::Vec<IamSchemasMessagesItem>> for IamSchemasMessages {
        fn from(value: ::std::vec::Vec<IamSchemasMessagesItem>) -> Self {
            Self(value)
        }
    }

    ///`IamSchemasMessagesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "documentation_url": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "source": {
    ///      "type": "object",
    ///      "properties": {
    ///        "pointer": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamSchemasMessagesItem {
        pub code: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub documentation_url: ::std::option::Option<::std::string::String>,
        pub message: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<IamSchemasMessagesItemSource>,
    }

    ///`IamSchemasMessagesItemSource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "pointer": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct IamSchemasMessagesItemSource {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pointer: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for IamSchemasMessagesItemSource {
        fn default() -> Self {
            Self {
                pointer: Default::default(),
            }
        }
    }

    ///`PagesApiResponseCollection`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result_info": {
    ///          "type": "object",
    ///          "properties": {
    ///            "count": {
    ///              "description": "Total number of results for the requested
    /// service.",
    ///              "examples": [
    ///                1
    ///              ],
    ///              "type": "number"
    ///            },
    ///            "page": {
    ///              "description": "Current page within paginated list of
    /// results.",
    ///              "examples": [
    ///                1
    ///              ],
    ///              "type": "number"
    ///            },
    ///            "per_page": {
    ///              "description": "Number of results per page of results.",
    ///              "examples": [
    ///                20
    ///              ],
    ///              "type": "number"
    ///            },
    ///            "total_count": {
    ///              "description": "Total results available without any search
    /// parameters.",
    ///              "examples": [
    ///                2000
    ///              ],
    ///              "type": "number"
    ///            },
    ///            "total_pages": {
    ///              "description": "The number of total pages in the entire
    /// result set.",
    ///              "examples": [
    ///                100
    ///              ],
    ///              "type": "number"
    ///            }
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesApiResponseCollection {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result_info: ::std::option::Option<PagesApiResponseCollectionResultInfo>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`PagesApiResponseCollectionResultInfo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "count": {
    ///      "description": "Total number of results for the requested
    /// service.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "page": {
    ///      "description": "Current page within paginated list of results.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "per_page": {
    ///      "description": "Number of results per page of results.",
    ///      "examples": [
    ///        20
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_count": {
    ///      "description": "Total results available without any search
    /// parameters.",
    ///      "examples": [
    ///        2000
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_pages": {
    ///      "description": "The number of total pages in the entire result
    /// set.",
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesApiResponseCollectionResultInfo {
        ///Total number of results for the requested service.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub count: ::std::option::Option<f64>,
        ///Current page within paginated list of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub page: ::std::option::Option<f64>,
        ///Number of results per page of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub per_page: ::std::option::Option<f64>,
        ///Total results available without any search parameters.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_count: ::std::option::Option<f64>,
        ///The number of total pages in the entire result set.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_pages: ::std::option::Option<f64>,
    }

    impl ::std::default::Default for PagesApiResponseCollectionResultInfo {
        fn default() -> Self {
            Self {
                count: Default::default(),
                page: Default::default(),
                per_page: Default::default(),
                total_count: Default::default(),
                total_pages: Default::default(),
            }
        }
    }

    ///`PagesApiResponseCommon`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "$ref": "#/components/schemas/pages_messages"
    ///    },
    ///    "messages": {
    ///      "$ref": "#/components/schemas/pages_messages"
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "enum": [
    ///        true
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesApiResponseCommon {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`PagesApiResponseCommonFailure`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "result",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "examples": [
    ///        [
    ///          {
    ///            "code": 7003,
    ///            "message": "No route for the URI"
    ///          }
    ///        ]
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/pages_messages"
    ///        }
    ///      ],
    ///      "minLength": 1
    ///    },
    ///    "messages": {
    ///      "examples": [
    ///        []
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/pages_messages"
    ///        }
    ///      ]
    ///    },
    ///    "result": {
    ///      "type": [
    ///        "object",
    ///        "null"
    ///      ],
    ///      "enum": [
    ///        null
    ///      ]
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean",
    ///      "enum": [
    ///        false
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesApiResponseCommonFailure {
        pub errors: ::std::vec::Vec<PagesApiResponseCommonFailureErrorsItem>,
        pub messages: PagesMessages,
        pub result: (),
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`PagesApiResponseCommonFailureErrorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "documentation_url": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "source": {
    ///      "type": "object",
    ///      "properties": {
    ///        "pointer": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesApiResponseCommonFailureErrorsItem {
        pub code: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub documentation_url: ::std::option::Option<::std::string::String>,
        pub message: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<PagesApiResponseCommonFailureErrorsItemSource>,
    }

    ///`PagesApiResponseCommonFailureErrorsItemSource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "pointer": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesApiResponseCommonFailureErrorsItemSource {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pointer: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for PagesApiResponseCommonFailureErrorsItemSource {
        fn default() -> Self {
            Self {
                pointer: Default::default(),
            }
        }
    }

    ///Configs for the project build process.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Configs for the project build process.",
    ///  "type": "object",
    ///  "required": [
    ///    "web_analytics_tag",
    ///    "web_analytics_token"
    ///  ],
    ///  "properties": {
    ///    "build_caching": {
    ///      "description": "Enable build caching for the project.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": [
    ///        "boolean",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "build_command": {
    ///      "description": "Command used to build project.",
    ///      "examples": [
    ///        "npm run build"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "destination_dir": {
    ///      "description": "Assets output directory of the build.",
    ///      "examples": [
    ///        "build"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "root_dir": {
    ///      "description": "Directory to run the command.",
    ///      "examples": [
    ///        "/"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "web_analytics_tag": {
    ///      "description": "The classifying tag for analytics.",
    ///      "examples": [
    ///        "cee1c73f6e4743d0b5e6bb1a0bcaabcc"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "web_analytics_token": {
    ///      "description": "The auth token for analytics.",
    ///      "examples": [
    ///        "021e1057c18547eca7b79f2516f06o7x"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-sensitive": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesBuildConfig {
        ///Enable build caching for the project.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub build_caching: ::std::option::Option<bool>,
        ///Command used to build project.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub build_command: ::std::option::Option<::std::string::String>,
        ///Assets output directory of the build.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub destination_dir: ::std::option::Option<::std::string::String>,
        ///Directory to run the command.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub root_dir: ::std::option::Option<::std::string::String>,
        ///The classifying tag for analytics.
        pub web_analytics_tag: ::std::option::Option<::std::string::String>,
        ///The auth token for analytics.
        pub web_analytics_token: ::std::option::Option<::std::string::String>,
    }

    ///`PagesDeployment`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "aliases",
    ///    "build_config",
    ///    "created_on",
    ///    "deployment_trigger",
    ///    "env_vars",
    ///    "environment",
    ///    "id",
    ///    "is_skipped",
    ///    "latest_stage",
    ///    "modified_on",
    ///    "project_id",
    ///    "project_name",
    ///    "short_id",
    ///    "source",
    ///    "stages",
    ///    "url"
    ///  ],
    ///  "properties": {
    ///    "aliases": {
    ///      "description": "A list of alias URLs pointing to this deployment.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        [
    ///          "https://branchname.projectname.pages.dev"
    ///        ]
    ///      ],
    ///      "type": [
    ///        "array",
    ///        "null"
    ///      ],
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "build_config": {
    ///      "$ref": "#/components/schemas/pages_build_config"
    ///    },
    ///    "created_on": {
    ///      "description": "When the deployment was created.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "2021-03-09T00:55:03.923456Z"
    ///      ],
    ///      "type": "string",
    ///      "format": "date-time",
    ///      "x-auditable": true
    ///    },
    ///    "deployment_trigger": {
    ///      "description": "Info about what caused the deployment.",
    ///      "readOnly": true,
    ///      "type": "object",
    ///      "required": [
    ///        "metadata",
    ///        "type"
    ///      ],
    ///      "properties": {
    ///        "metadata": {
    ///          "description": "Additional info about the trigger.",
    ///          "type": "object",
    ///          "required": [
    ///            "branch",
    ///            "commit_dirty",
    ///            "commit_hash",
    ///            "commit_message"
    ///          ],
    ///          "properties": {
    ///            "branch": {
    ///              "description": "Where the trigger happened.",
    ///              "examples": [
    ///                "main"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            },
    ///            "commit_dirty": {
    ///              "description": "Whether the deployment trigger commit was
    /// dirty.",
    ///              "examples": [
    ///                false
    ///              ],
    ///              "type": "boolean",
    ///              "x-auditable": true
    ///            },
    ///            "commit_hash": {
    ///              "description": "Hash of the deployment trigger commit.",
    ///              "examples": [
    ///                "ad9ccd918a81025731e10e40267e11273a263421"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            },
    ///            "commit_message": {
    ///              "description": "Message of the deployment trigger commit.",
    ///              "examples": [
    ///                "Update index.html"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            }
    ///          }
    ///        },
    ///        "type": {
    ///          "description": "What caused the deployment.",
    ///          "examples": [
    ///            "ad_hoc"
    ///          ],
    ///          "type": "string",
    ///          "enum": [
    ///            "github:push",
    ///            "ad_hoc",
    ///            "deploy_hook"
    ///          ],
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    },
    ///    "env_vars": {
    ///      "$ref": "#/components/schemas/pages_env_vars"
    ///    },
    ///    "environment": {
    ///      "description": "Type of deploy.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "preview"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "preview",
    ///        "production"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "id": {
    ///      "description": "Id of the deployment.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "f64788e9-fccd-4d4a-a28a-cb84f88f6"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "is_skipped": {
    ///      "description": "If the deployment has been skipped.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "latest_stage": {
    ///      "$ref": "#/components/schemas/pages_stage"
    ///    },
    ///    "modified_on": {
    ///      "description": "When the deployment was last modified.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "2021-03-09T00:58:59.045655"
    ///      ],
    ///      "type": "string",
    ///      "format": "date-time",
    ///      "x-auditable": true
    ///    },
    ///    "project_id": {
    ///      "description": "Id of the project.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "7b162ea7-7367-4d67-bcde-1160995d5"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "project_name": {
    ///      "$ref": "#/components/schemas/pages_project_name"
    ///    },
    ///    "short_id": {
    ///      "description": "Short Id (8 character) of the deployment.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "f64788e9"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "source": {
    ///      "$ref": "#/components/schemas/pages_source"
    ///    },
    ///    "stages": {
    ///      "description": "List of past stages.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        [
    ///          {
    ///            "ended_on": "2021-06-03T15:39:03.134378Z",
    ///            "name": "queued",
    ///            "started_on": "2021-06-03T15:38:15.608194Z",
    ///            "status": "active"
    ///          },
    ///          {
    ///            "ended_on": null,
    ///            "name": "initialize",
    ///            "started_on": null,
    ///            "status": "idle"
    ///          },
    ///          {
    ///            "ended_on": null,
    ///            "name": "clone_repo",
    ///            "started_on": null,
    ///            "status": "idle"
    ///          },
    ///          {
    ///            "ended_on": null,
    ///            "name": "build",
    ///            "started_on": null,
    ///            "status": "idle"
    ///          },
    ///          {
    ///            "ended_on": null,
    ///            "name": "deploy",
    ///            "started_on": null,
    ///            "status": "idle"
    ///          }
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/pages_stage"
    ///      }
    ///    },
    ///    "url": {
    ///      "description": "The live URL to view this deployment.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "https://f64788e9.ninjakittens.pages.dev"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "uses_functions": {
    ///      "description": "Whether the deployment uses functions.",
    ///      "readOnly": true,
    ///      "type": [
    ///        "boolean",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeployment {
        ///A list of alias URLs pointing to this deployment.
        pub aliases: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
        pub build_config: PagesBuildConfig,
        ///When the deployment was created.
        pub created_on: ::chrono::DateTime<::chrono::offset::Utc>,
        pub deployment_trigger: PagesDeploymentDeploymentTrigger,
        pub env_vars: PagesEnvVars,
        ///Type of deploy.
        pub environment: PagesDeploymentEnvironment,
        ///Id of the deployment.
        pub id: ::std::string::String,
        ///If the deployment has been skipped.
        pub is_skipped: bool,
        pub latest_stage: PagesStage,
        ///When the deployment was last modified.
        pub modified_on: ::chrono::DateTime<::chrono::offset::Utc>,
        ///Id of the project.
        pub project_id: ::std::string::String,
        pub project_name: PagesProjectName,
        ///Short Id (8 character) of the deployment.
        pub short_id: ::std::string::String,
        pub source: PagesSource,
        ///List of past stages.
        pub stages: ::std::vec::Vec<PagesStage>,
        ///The live URL to view this deployment.
        pub url: ::std::string::String,
        ///Whether the deployment uses functions.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub uses_functions: ::std::option::Option<bool>,
    }

    ///`PagesDeploymentConfigValues`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "always_use_latest_compatibility_date",
    ///    "build_image_major_version",
    ///    "compatibility_date",
    ///    "compatibility_flags",
    ///    "env_vars",
    ///    "fail_open",
    ///    "usage_model"
    ///  ],
    ///  "properties": {
    ///    "ai_bindings": {
    ///      "description": "Constellation bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "AI_BINDING": {
    ///            "project_id": "some-project-id"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "AI binding.",
    ///        "type": "object",
    ///        "required": [
    ///          "project_id"
    ///        ],
    ///        "properties": {
    ///          "project_id": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "always_use_latest_compatibility_date": {
    ///      "description": "Whether to always use the latest compatibility date
    /// for Pages Functions.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "analytics_engine_datasets": {
    ///      "description": "Analytics Engine bindings used for Pages
    /// Functions.",
    ///      "examples": [
    ///        {
    ///          "ANALYTICS_ENGINE_BINDING": {
    ///            "dataset": "api_analytics"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Analytics Engine binding.",
    ///        "type": "object",
    ///        "required": [
    ///          "dataset"
    ///        ],
    ///        "properties": {
    ///          "dataset": {
    ///            "description": "Name of the dataset.",
    ///            "examples": [
    ///              "api_analytics"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "browsers": {
    ///      "description": "Browser bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "BROWSER": {}
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Browser binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "x-stainless-empty-object": true
    ///      }
    ///    },
    ///    "build_image_major_version": {
    ///      "description": "The major version of the build image to use for
    /// Pages Functions.",
    ///      "examples": [
    ///        3
    ///      ],
    ///      "type": "integer",
    ///      "x-auditable": true
    ///    },
    ///    "compatibility_date": {
    ///      "description": "Compatibility date used for Pages Functions.",
    ///      "examples": [
    ///        "2025-01-01"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true,
    ///      "x-stainless-terraform-configurability": "computed_optional"
    ///    },
    ///    "compatibility_flags": {
    ///      "description": "Compatibility flags used for Pages Functions.",
    ///      "examples": [
    ///        [
    ///          "url_standard"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "d1_databases": {
    ///      "description": "D1 databases used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "D1_BINDING": {
    ///            "id": "445e2955-951a-43f8-a35b-a4d0c8138f63"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "D1 binding.",
    ///        "type": "object",
    ///        "required": [
    ///          "id"
    ///        ],
    ///        "properties": {
    ///          "id": {
    ///            "description": "UUID of the D1 database.",
    ///            "examples": [
    ///              "445e2955-951a-43f8-a35b-a4d0c8138f63"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "durable_object_namespaces": {
    ///      "description": "Durable Object namespaces used for Pages
    /// Functions.",
    ///      "examples": [
    ///        {
    ///          "DO_BINDING": {
    ///            "namespace_id": "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Durable Object binding.",
    ///        "type": "object",
    ///        "required": [
    ///          "namespace_id"
    ///        ],
    ///        "properties": {
    ///          "namespace_id": {
    ///            "description": "ID of the Durable Object namespace.",
    ///            "examples": [
    ///              "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "env_vars": {
    ///      "$ref": "#/components/schemas/pages_env_vars"
    ///    },
    ///    "fail_open": {
    ///      "description": "Whether to fail open when the deployment config
    /// cannot be applied.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "hyperdrive_bindings": {
    ///      "description": "Hyperdrive bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "HYPERDRIVE": {
    ///            "id": "a76a99bc342644deb02c38d66082262a"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Hyperdrive binding.",
    ///        "type": "object",
    ///        "required": [
    ///          "id"
    ///        ],
    ///        "properties": {
    ///          "id": {
    ///            "examples": [
    ///              "a76a99bc342644deb02c38d66082262a"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "kv_namespaces": {
    ///      "description": "KV namespaces used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "KV_BINDING": {
    ///            "namespace_id": "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "KV namespace binding.",
    ///        "type": "object",
    ///        "required": [
    ///          "namespace_id"
    ///        ],
    ///        "properties": {
    ///          "namespace_id": {
    ///            "description": "ID of the KV namespace.",
    ///            "examples": [
    ///              "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "limits": {
    ///      "description": "Limits for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "cpu_ms": 100
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "required": [
    ///        "cpu_ms"
    ///      ],
    ///      "properties": {
    ///        "cpu_ms": {
    ///          "description": "CPU time limit in milliseconds.",
    ///          "examples": [
    ///            100
    ///          ],
    ///          "type": "integer",
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    },
    ///    "mtls_certificates": {
    ///      "description": "mTLS bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "MTLS": {
    ///            "certificate_id": "d7cdd17c-916f-4cb7-aabe-585eb382ec4e"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "mTLS binding.",
    ///        "type": "object",
    ///        "required": [
    ///          "certificate_id"
    ///        ],
    ///        "properties": {
    ///          "certificate_id": {
    ///            "examples": [
    ///              "d7cdd17c-916f-4cb7-aabe-585eb382ec4e"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "placement": {
    ///      "description": "Placement setting used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "mode": "smart"
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "required": [
    ///        "mode"
    ///      ],
    ///      "properties": {
    ///        "mode": {
    ///          "description": "Placement mode.",
    ///          "examples": [
    ///            "smart"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    },
    ///    "queue_producers": {
    ///      "description": "Queue Producer bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "QUEUE_PRODUCER_BINDING": {
    ///            "name": "some-queue"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Queue Producer binding.",
    ///        "type": "object",
    ///        "required": [
    ///          "name"
    ///        ],
    ///        "properties": {
    ///          "name": {
    ///            "description": "Name of the Queue.",
    ///            "examples": [
    ///              "some-queue"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "r2_buckets": {
    ///      "description": "R2 buckets used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "R2_BINDING": {
    ///            "name": "some-bucket"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "R2 binding.",
    ///        "type": "object",
    ///        "required": [
    ///          "name"
    ///        ],
    ///        "properties": {
    ///          "jurisdiction": {
    ///            "description": "Jurisdiction of the R2 bucket.",
    ///            "examples": [
    ///              "eu"
    ///            ],
    ///            "type": [
    ///              "string",
    ///              "null"
    ///            ],
    ///            "x-auditable": true
    ///          },
    ///          "name": {
    ///            "description": "Name of the R2 bucket.",
    ///            "examples": [
    ///              "some-bucket"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "services": {
    ///      "description": "Services used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "SERVICE_BINDING": {
    ///            "entrypoint": "MyHandler",
    ///            "environment": "production",
    ///            "service": "example-worker"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Service binding.",
    ///        "type": "object",
    ///        "required": [
    ///          "environment",
    ///          "service"
    ///        ],
    ///        "properties": {
    ///          "entrypoint": {
    ///            "description": "The entrypoint to bind to.",
    ///            "type": [
    ///              "string",
    ///              "null"
    ///            ],
    ///            "x-auditable": true
    ///          },
    ///          "environment": {
    ///            "description": "The Service environment.",
    ///            "type": "string",
    ///            "x-auditable": true
    ///          },
    ///          "service": {
    ///            "description": "The Service name.",
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "usage_model": {
    ///      "description": "The usage model for Pages Functions.",
    ///      "deprecated": true,
    ///      "examples": [
    ///        "standard"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "standard",
    ///        "bundled",
    ///        "unbound"
    ///      ],
    ///      "x-auditable": true,
    ///      "x-stainless-deprecation-message": "All new projects now use the
    /// Standard usage model."
    ///    },
    ///    "vectorize_bindings": {
    ///      "description": "Vectorize bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "VECTORIZE": {
    ///            "index_name": "my_index"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Vectorize binding.",
    ///        "type": "object",
    ///        "required": [
    ///          "index_name"
    ///        ],
    ///        "properties": {
    ///          "index_name": {
    ///            "examples": [
    ///              "my_index"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "wrangler_config_hash": {
    ///      "description": "Hash of the Wrangler configuration used for the
    /// deployment.",
    ///      "examples": [
    ///        "abc123def456"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValues {
        ///Constellation bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub ai_bindings: ::std::collections::HashMap<
            ::std::string::String,
            PagesDeploymentConfigValuesAiBindingsValue,
        >,
        ///Whether to always use the latest compatibility date for Pages
        /// Functions.
        pub always_use_latest_compatibility_date: bool,
        ///Analytics Engine bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub analytics_engine_datasets: ::std::collections::HashMap<
            ::std::string::String,
            PagesDeploymentConfigValuesAnalyticsEngineDatasetsValue,
        >,
        ///Browser bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub browsers: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        >,
        ///The major version of the build image to use for Pages Functions.
        pub build_image_major_version: i64,
        ///Compatibility date used for Pages Functions.
        pub compatibility_date: ::std::string::String,
        ///Compatibility flags used for Pages Functions.
        pub compatibility_flags: ::std::vec::Vec<::std::string::String>,
        ///D1 databases used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub d1_databases: ::std::collections::HashMap<
            ::std::string::String,
            PagesDeploymentConfigValuesD1DatabasesValue,
        >,
        ///Durable Object namespaces used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub durable_object_namespaces: ::std::collections::HashMap<
            ::std::string::String,
            PagesDeploymentConfigValuesDurableObjectNamespacesValue,
        >,
        pub env_vars: PagesEnvVars,
        ///Whether to fail open when the deployment config cannot be applied.
        pub fail_open: bool,
        ///Hyperdrive bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub hyperdrive_bindings: ::std::collections::HashMap<
            ::std::string::String,
            PagesDeploymentConfigValuesHyperdriveBindingsValue,
        >,
        ///KV namespaces used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub kv_namespaces: ::std::collections::HashMap<
            ::std::string::String,
            PagesDeploymentConfigValuesKvNamespacesValue,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub limits: ::std::option::Option<PagesDeploymentConfigValuesLimits>,
        ///mTLS bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub mtls_certificates: ::std::collections::HashMap<
            ::std::string::String,
            PagesDeploymentConfigValuesMtlsCertificatesValue,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub placement: ::std::option::Option<PagesDeploymentConfigValuesPlacement>,
        ///Queue Producer bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub queue_producers: ::std::collections::HashMap<
            ::std::string::String,
            PagesDeploymentConfigValuesQueueProducersValue,
        >,
        ///R2 buckets used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub r2_buckets: ::std::collections::HashMap<
            ::std::string::String,
            PagesDeploymentConfigValuesR2BucketsValue,
        >,
        ///Services used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub services: ::std::collections::HashMap<
            ::std::string::String,
            PagesDeploymentConfigValuesServicesValue,
        >,
        ///The usage model for Pages Functions.
        pub usage_model: PagesDeploymentConfigValuesUsageModel,
        ///Vectorize bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub vectorize_bindings: ::std::collections::HashMap<
            ::std::string::String,
            PagesDeploymentConfigValuesVectorizeBindingsValue,
        >,
        ///Hash of the Wrangler configuration used for the deployment.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub wrangler_config_hash: ::std::option::Option<::std::string::String>,
    }

    ///AI binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "AI binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "project_id"
    ///  ],
    ///  "properties": {
    ///    "project_id": {
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesAiBindingsValue {
        pub project_id: ::std::string::String,
    }

    ///Analytics Engine binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Analytics Engine binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "dataset"
    ///  ],
    ///  "properties": {
    ///    "dataset": {
    ///      "description": "Name of the dataset.",
    ///      "examples": [
    ///        "api_analytics"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesAnalyticsEngineDatasetsValue {
        ///Name of the dataset.
        pub dataset: ::std::string::String,
    }

    ///D1 binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "D1 binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "id"
    ///  ],
    ///  "properties": {
    ///    "id": {
    ///      "description": "UUID of the D1 database.",
    ///      "examples": [
    ///        "445e2955-951a-43f8-a35b-a4d0c8138f63"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesD1DatabasesValue {
        ///UUID of the D1 database.
        pub id: ::std::string::String,
    }

    ///Durable Object binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Durable Object binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "namespace_id"
    ///  ],
    ///  "properties": {
    ///    "namespace_id": {
    ///      "description": "ID of the Durable Object namespace.",
    ///      "examples": [
    ///        "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesDurableObjectNamespacesValue {
        ///ID of the Durable Object namespace.
        pub namespace_id: ::std::string::String,
    }

    ///Hyperdrive binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Hyperdrive binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "id"
    ///  ],
    ///  "properties": {
    ///    "id": {
    ///      "examples": [
    ///        "a76a99bc342644deb02c38d66082262a"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesHyperdriveBindingsValue {
        pub id: ::std::string::String,
    }

    ///KV namespace binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "KV namespace binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "namespace_id"
    ///  ],
    ///  "properties": {
    ///    "namespace_id": {
    ///      "description": "ID of the KV namespace.",
    ///      "examples": [
    ///        "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesKvNamespacesValue {
        ///ID of the KV namespace.
        pub namespace_id: ::std::string::String,
    }

    ///Limits for Pages Functions.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Limits for Pages Functions.",
    ///  "examples": [
    ///    {
    ///      "cpu_ms": 100
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "required": [
    ///    "cpu_ms"
    ///  ],
    ///  "properties": {
    ///    "cpu_ms": {
    ///      "description": "CPU time limit in milliseconds.",
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "integer",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesLimits {
        ///CPU time limit in milliseconds.
        pub cpu_ms: i64,
    }

    ///mTLS binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "mTLS binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "certificate_id"
    ///  ],
    ///  "properties": {
    ///    "certificate_id": {
    ///      "examples": [
    ///        "d7cdd17c-916f-4cb7-aabe-585eb382ec4e"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesMtlsCertificatesValue {
        pub certificate_id: ::std::string::String,
    }

    ///Placement setting used for Pages Functions.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Placement setting used for Pages Functions.",
    ///  "examples": [
    ///    {
    ///      "mode": "smart"
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "required": [
    ///    "mode"
    ///  ],
    ///  "properties": {
    ///    "mode": {
    ///      "description": "Placement mode.",
    ///      "examples": [
    ///        "smart"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesPlacement {
        ///Placement mode.
        pub mode: ::std::string::String,
    }

    ///Queue Producer binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Queue Producer binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "name": {
    ///      "description": "Name of the Queue.",
    ///      "examples": [
    ///        "some-queue"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesQueueProducersValue {
        ///Name of the Queue.
        pub name: ::std::string::String,
    }

    ///R2 binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "R2 binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "jurisdiction": {
    ///      "description": "Jurisdiction of the R2 bucket.",
    ///      "examples": [
    ///        "eu"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "name": {
    ///      "description": "Name of the R2 bucket.",
    ///      "examples": [
    ///        "some-bucket"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesR2BucketsValue {
        ///Jurisdiction of the R2 bucket.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub jurisdiction: ::std::option::Option<::std::string::String>,
        ///Name of the R2 bucket.
        pub name: ::std::string::String,
    }

    ///`PagesDeploymentConfigValuesRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "ai_bindings": {
    ///      "description": "Constellation bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "AI_BINDING": {
    ///            "project_id": "some-project-id"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "AI binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "required": [
    ///          "project_id"
    ///        ],
    ///        "properties": {
    ///          "project_id": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "always_use_latest_compatibility_date": {
    ///      "description": "Whether to always use the latest compatibility date
    /// for Pages Functions.",
    ///      "default": false,
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "analytics_engine_datasets": {
    ///      "description": "Analytics Engine bindings used for Pages
    /// Functions.",
    ///      "examples": [
    ///        {
    ///          "ANALYTICS_ENGINE_BINDING": {
    ///            "dataset": "api_analytics"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Analytics Engine binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "required": [
    ///          "dataset"
    ///        ],
    ///        "properties": {
    ///          "dataset": {
    ///            "description": "Name of the dataset.",
    ///            "examples": [
    ///              "api_analytics"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "browsers": {
    ///      "description": "Browser bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "BROWSER": {}
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Browser binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "x-stainless-empty-object": true
    ///      }
    ///    },
    ///    "build_image_major_version": {
    ///      "description": "The major version of the build image to use for
    /// Pages Functions.",
    ///      "default": 3,
    ///      "examples": [
    ///        3
    ///      ],
    ///      "type": "integer",
    ///      "x-auditable": true
    ///    },
    ///    "compatibility_date": {
    ///      "description": "Compatibility date used for Pages Functions.",
    ///      "examples": [
    ///        "2025-01-01"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true,
    ///      "x-stainless-terraform-configurability": "computed_optional"
    ///    },
    ///    "compatibility_flags": {
    ///      "description": "Compatibility flags used for Pages Functions.",
    ///      "examples": [
    ///        [
    ///          "url_standard"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "d1_databases": {
    ///      "description": "D1 databases used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "D1_BINDING": {
    ///            "id": "445e2955-951a-43f8-a35b-a4d0c8138f63"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "D1 binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "required": [
    ///          "id"
    ///        ],
    ///        "properties": {
    ///          "id": {
    ///            "description": "UUID of the D1 database.",
    ///            "examples": [
    ///              "445e2955-951a-43f8-a35b-a4d0c8138f63"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "durable_object_namespaces": {
    ///      "description": "Durable Object namespaces used for Pages
    /// Functions.",
    ///      "examples": [
    ///        {
    ///          "DO_BINDING": {
    ///            "namespace_id": "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Durable Object binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "required": [
    ///          "namespace_id"
    ///        ],
    ///        "properties": {
    ///          "namespace_id": {
    ///            "description": "ID of the Durable Object namespace.",
    ///            "examples": [
    ///              "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "env_vars": {
    ///      "description": "Environment variables used for builds and Pages
    /// Functions.",
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "oneOf": [
    ///          {
    ///            "$ref": "#/components/schemas/pages_plain_text_env_var"
    ///          },
    ///          {
    ///            "$ref": "#/components/schemas/pages_secret_text_env_var"
    ///          }
    ///        ]
    ///      }
    ///    },
    ///    "fail_open": {
    ///      "description": "Whether to fail open when the deployment config
    /// cannot be applied.",
    ///      "default": true,
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "hyperdrive_bindings": {
    ///      "description": "Hyperdrive bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "HYPERDRIVE": {
    ///            "id": "a76a99bc342644deb02c38d66082262a"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Hyperdrive binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "required": [
    ///          "id"
    ///        ],
    ///        "properties": {
    ///          "id": {
    ///            "examples": [
    ///              "a76a99bc342644deb02c38d66082262a"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "kv_namespaces": {
    ///      "description": "KV namespaces used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "KV_BINDING": {
    ///            "namespace_id": "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "KV namespace binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "required": [
    ///          "namespace_id"
    ///        ],
    ///        "properties": {
    ///          "namespace_id": {
    ///            "description": "ID of the KV namespace.",
    ///            "examples": [
    ///              "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "limits": {
    ///      "description": "Limits for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "cpu_ms": 100
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "required": [
    ///        "cpu_ms"
    ///      ],
    ///      "properties": {
    ///        "cpu_ms": {
    ///          "description": "CPU time limit in milliseconds.",
    ///          "examples": [
    ///            100
    ///          ],
    ///          "type": "integer",
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    },
    ///    "mtls_certificates": {
    ///      "description": "mTLS bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "MTLS": {
    ///            "certificate_id": "d7cdd17c-916f-4cb7-aabe-585eb382ec4e"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "mTLS binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "required": [
    ///          "certificate_id"
    ///        ],
    ///        "properties": {
    ///          "certificate_id": {
    ///            "examples": [
    ///              "d7cdd17c-916f-4cb7-aabe-585eb382ec4e"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "placement": {
    ///      "description": "Placement setting used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "mode": "smart"
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "required": [
    ///        "mode"
    ///      ],
    ///      "properties": {
    ///        "mode": {
    ///          "description": "Placement mode.",
    ///          "examples": [
    ///            "smart"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    },
    ///    "queue_producers": {
    ///      "description": "Queue Producer bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "QUEUE_PRODUCER_BINDING": {
    ///            "name": "some-queue"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Queue Producer binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "required": [
    ///          "name"
    ///        ],
    ///        "properties": {
    ///          "name": {
    ///            "description": "Name of the Queue.",
    ///            "examples": [
    ///              "some-queue"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "r2_buckets": {
    ///      "description": "R2 buckets used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "R2_BINDING": {
    ///            "name": "some-bucket"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "R2 binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "required": [
    ///          "name"
    ///        ],
    ///        "properties": {
    ///          "jurisdiction": {
    ///            "description": "Jurisdiction of the R2 bucket.",
    ///            "examples": [
    ///              "eu"
    ///            ],
    ///            "type": [
    ///              "string",
    ///              "null"
    ///            ],
    ///            "x-auditable": true
    ///          },
    ///          "name": {
    ///            "description": "Name of the R2 bucket.",
    ///            "examples": [
    ///              "some-bucket"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "services": {
    ///      "description": "Services used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "SERVICE_BINDING": {
    ///            "entrypoint": "MyHandler",
    ///            "environment": "production",
    ///            "service": "example-worker"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Service binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "required": [
    ///          "service"
    ///        ],
    ///        "properties": {
    ///          "entrypoint": {
    ///            "description": "The entrypoint to bind to.",
    ///            "type": [
    ///              "string",
    ///              "null"
    ///            ],
    ///            "x-auditable": true
    ///          },
    ///          "environment": {
    ///            "description": "The Service environment.",
    ///            "type": "string",
    ///            "x-auditable": true
    ///          },
    ///          "service": {
    ///            "description": "The Service name.",
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "usage_model": {
    ///      "description": "The usage model for Pages Functions.",
    ///      "default": "standard",
    ///      "deprecated": true,
    ///      "examples": [
    ///        "standard"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "standard",
    ///        "bundled",
    ///        "unbound"
    ///      ],
    ///      "x-auditable": true,
    ///      "x-stainless-deprecation-message": "All new projects now use the
    /// Standard usage model."
    ///    },
    ///    "vectorize_bindings": {
    ///      "description": "Vectorize bindings used for Pages Functions.",
    ///      "examples": [
    ///        {
    ///          "VECTORIZE": {
    ///            "index_name": "my_index"
    ///          }
    ///        }
    ///      ],
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "description": "Vectorize binding.",
    ///        "type": [
    ///          "object",
    ///          "null"
    ///        ],
    ///        "required": [
    ///          "index_name"
    ///        ],
    ///        "properties": {
    ///          "index_name": {
    ///            "examples": [
    ///              "my_index"
    ///            ],
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "wrangler_config_hash": {
    ///      "description": "Hash of the Wrangler configuration used for the
    /// deployment.",
    ///      "examples": [
    ///        "abc123def456"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequest {
        ///Constellation bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub ai_bindings: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestAiBindingsValue>,
        >,
        ///Whether to always use the latest compatibility date for Pages
        /// Functions.
        #[serde(default)]
        pub always_use_latest_compatibility_date: bool,
        ///Analytics Engine bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub analytics_engine_datasets: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestAnalyticsEngineDatasetsValue>,
        >,
        ///Browser bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub browsers: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        >,
        ///The major version of the build image to use for Pages Functions.
        #[serde(default = "defaults::default_u64::<i64, 3>")]
        pub build_image_major_version: i64,
        ///Compatibility date used for Pages Functions.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub compatibility_date: ::std::option::Option<::std::string::String>,
        ///Compatibility flags used for Pages Functions.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub compatibility_flags: ::std::vec::Vec<::std::string::String>,
        ///D1 databases used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub d1_databases: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestD1DatabasesValue>,
        >,
        ///Durable Object namespaces used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub durable_object_namespaces: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestDurableObjectNamespacesValue>,
        >,
        ///Environment variables used for builds and Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub env_vars: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestEnvVarsValue>,
        >,
        ///Whether to fail open when the deployment config cannot be applied.
        #[serde(default = "defaults::default_bool::<true>")]
        pub fail_open: bool,
        ///Hyperdrive bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub hyperdrive_bindings: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestHyperdriveBindingsValue>,
        >,
        ///KV namespaces used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub kv_namespaces: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestKvNamespacesValue>,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub limits: ::std::option::Option<PagesDeploymentConfigValuesRequestLimits>,
        ///mTLS bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub mtls_certificates: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestMtlsCertificatesValue>,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub placement: ::std::option::Option<PagesDeploymentConfigValuesRequestPlacement>,
        ///Queue Producer bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub queue_producers: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestQueueProducersValue>,
        >,
        ///R2 buckets used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub r2_buckets: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestR2BucketsValue>,
        >,
        ///Services used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub services: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestServicesValue>,
        >,
        ///The usage model for Pages Functions.
        #[serde(default = "defaults::pages_deployment_config_values_request_usage_model")]
        pub usage_model: PagesDeploymentConfigValuesRequestUsageModel,
        ///Vectorize bindings used for Pages Functions.
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub vectorize_bindings: ::std::collections::HashMap<
            ::std::string::String,
            ::std::option::Option<PagesDeploymentConfigValuesRequestVectorizeBindingsValue>,
        >,
        ///Hash of the Wrangler configuration used for the deployment.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub wrangler_config_hash: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for PagesDeploymentConfigValuesRequest {
        fn default() -> Self {
            Self {
                ai_bindings: Default::default(),
                always_use_latest_compatibility_date: Default::default(),
                analytics_engine_datasets: Default::default(),
                browsers: Default::default(),
                build_image_major_version: defaults::default_u64::<i64, 3>(),
                compatibility_date: Default::default(),
                compatibility_flags: Default::default(),
                d1_databases: Default::default(),
                durable_object_namespaces: Default::default(),
                env_vars: Default::default(),
                fail_open: defaults::default_bool::<true>(),
                hyperdrive_bindings: Default::default(),
                kv_namespaces: Default::default(),
                limits: Default::default(),
                mtls_certificates: Default::default(),
                placement: Default::default(),
                queue_producers: Default::default(),
                r2_buckets: Default::default(),
                services: Default::default(),
                usage_model: defaults::pages_deployment_config_values_request_usage_model(),
                vectorize_bindings: Default::default(),
                wrangler_config_hash: Default::default(),
            }
        }
    }

    ///AI binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "AI binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "project_id"
    ///  ],
    ///  "properties": {
    ///    "project_id": {
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestAiBindingsValue {
        pub project_id: ::std::string::String,
    }

    ///Analytics Engine binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Analytics Engine binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "dataset"
    ///  ],
    ///  "properties": {
    ///    "dataset": {
    ///      "description": "Name of the dataset.",
    ///      "examples": [
    ///        "api_analytics"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestAnalyticsEngineDatasetsValue {
        ///Name of the dataset.
        pub dataset: ::std::string::String,
    }

    ///D1 binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "D1 binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "id"
    ///  ],
    ///  "properties": {
    ///    "id": {
    ///      "description": "UUID of the D1 database.",
    ///      "examples": [
    ///        "445e2955-951a-43f8-a35b-a4d0c8138f63"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestD1DatabasesValue {
        ///UUID of the D1 database.
        pub id: ::std::string::String,
    }

    ///Durable Object binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Durable Object binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "namespace_id"
    ///  ],
    ///  "properties": {
    ///    "namespace_id": {
    ///      "description": "ID of the Durable Object namespace.",
    ///      "examples": [
    ///        "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestDurableObjectNamespacesValue {
        ///ID of the Durable Object namespace.
        pub namespace_id: ::std::string::String,
    }

    ///`PagesDeploymentConfigValuesRequestEnvVarsValue`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "oneOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_plain_text_env_var"
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/pages_secret_text_env_var"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum PagesDeploymentConfigValuesRequestEnvVarsValue {
        PlainTextEnvVar(PagesPlainTextEnvVar),
        SecretTextEnvVar(PagesSecretTextEnvVar),
    }

    impl ::std::convert::From<PagesPlainTextEnvVar> for PagesDeploymentConfigValuesRequestEnvVarsValue {
        fn from(value: PagesPlainTextEnvVar) -> Self {
            Self::PlainTextEnvVar(value)
        }
    }

    impl ::std::convert::From<PagesSecretTextEnvVar>
        for PagesDeploymentConfigValuesRequestEnvVarsValue
    {
        fn from(value: PagesSecretTextEnvVar) -> Self {
            Self::SecretTextEnvVar(value)
        }
    }

    ///Hyperdrive binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Hyperdrive binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "id"
    ///  ],
    ///  "properties": {
    ///    "id": {
    ///      "examples": [
    ///        "a76a99bc342644deb02c38d66082262a"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestHyperdriveBindingsValue {
        pub id: ::std::string::String,
    }

    ///KV namespace binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "KV namespace binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "namespace_id"
    ///  ],
    ///  "properties": {
    ///    "namespace_id": {
    ///      "description": "ID of the KV namespace.",
    ///      "examples": [
    ///        "5eb63bbbe01eeed093cb22bb8f5acdc3"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestKvNamespacesValue {
        ///ID of the KV namespace.
        pub namespace_id: ::std::string::String,
    }

    ///Limits for Pages Functions.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Limits for Pages Functions.",
    ///  "examples": [
    ///    {
    ///      "cpu_ms": 100
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "required": [
    ///    "cpu_ms"
    ///  ],
    ///  "properties": {
    ///    "cpu_ms": {
    ///      "description": "CPU time limit in milliseconds.",
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "integer",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestLimits {
        ///CPU time limit in milliseconds.
        pub cpu_ms: i64,
    }

    ///mTLS binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "mTLS binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "certificate_id"
    ///  ],
    ///  "properties": {
    ///    "certificate_id": {
    ///      "examples": [
    ///        "d7cdd17c-916f-4cb7-aabe-585eb382ec4e"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestMtlsCertificatesValue {
        pub certificate_id: ::std::string::String,
    }

    ///Placement setting used for Pages Functions.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Placement setting used for Pages Functions.",
    ///  "examples": [
    ///    {
    ///      "mode": "smart"
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "required": [
    ///    "mode"
    ///  ],
    ///  "properties": {
    ///    "mode": {
    ///      "description": "Placement mode.",
    ///      "examples": [
    ///        "smart"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestPlacement {
        ///Placement mode.
        pub mode: ::std::string::String,
    }

    ///Queue Producer binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Queue Producer binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "name": {
    ///      "description": "Name of the Queue.",
    ///      "examples": [
    ///        "some-queue"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestQueueProducersValue {
        ///Name of the Queue.
        pub name: ::std::string::String,
    }

    ///R2 binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "R2 binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "jurisdiction": {
    ///      "description": "Jurisdiction of the R2 bucket.",
    ///      "examples": [
    ///        "eu"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "name": {
    ///      "description": "Name of the R2 bucket.",
    ///      "examples": [
    ///        "some-bucket"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestR2BucketsValue {
        ///Jurisdiction of the R2 bucket.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub jurisdiction: ::std::option::Option<::std::string::String>,
        ///Name of the R2 bucket.
        pub name: ::std::string::String,
    }

    ///Service binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Service binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "service"
    ///  ],
    ///  "properties": {
    ///    "entrypoint": {
    ///      "description": "The entrypoint to bind to.",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "environment": {
    ///      "description": "The Service environment.",
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "service": {
    ///      "description": "The Service name.",
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestServicesValue {
        ///The entrypoint to bind to.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub entrypoint: ::std::option::Option<::std::string::String>,
        ///The Service environment.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub environment: ::std::option::Option<::std::string::String>,
        ///The Service name.
        pub service: ::std::string::String,
    }

    ///The usage model for Pages Functions.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The usage model for Pages Functions.",
    ///  "default": "standard",
    ///  "deprecated": true,
    ///  "examples": [
    ///    "standard"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "standard",
    ///    "bundled",
    ///    "unbound"
    ///  ],
    ///  "x-auditable": true,
    ///  "x-stainless-deprecation-message": "All new projects now use the
    /// Standard usage model."
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
    pub enum PagesDeploymentConfigValuesRequestUsageModel {
        #[serde(rename = "standard")]
        Standard,
        #[serde(rename = "bundled")]
        Bundled,
        #[serde(rename = "unbound")]
        Unbound,
    }

    impl ::std::fmt::Display for PagesDeploymentConfigValuesRequestUsageModel {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Standard => f.write_str("standard"),
                Self::Bundled => f.write_str("bundled"),
                Self::Unbound => f.write_str("unbound"),
            }
        }
    }

    impl ::std::str::FromStr for PagesDeploymentConfigValuesRequestUsageModel {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "standard" => Ok(Self::Standard),
                "bundled" => Ok(Self::Bundled),
                "unbound" => Ok(Self::Unbound),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesDeploymentConfigValuesRequestUsageModel {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for PagesDeploymentConfigValuesRequestUsageModel
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for PagesDeploymentConfigValuesRequestUsageModel
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for PagesDeploymentConfigValuesRequestUsageModel {
        fn default() -> Self {
            PagesDeploymentConfigValuesRequestUsageModel::Standard
        }
    }

    ///Vectorize binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Vectorize binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "index_name"
    ///  ],
    ///  "properties": {
    ///    "index_name": {
    ///      "examples": [
    ///        "my_index"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesRequestVectorizeBindingsValue {
        pub index_name: ::std::string::String,
    }

    ///Service binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Service binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "environment",
    ///    "service"
    ///  ],
    ///  "properties": {
    ///    "entrypoint": {
    ///      "description": "The entrypoint to bind to.",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "environment": {
    ///      "description": "The Service environment.",
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "service": {
    ///      "description": "The Service name.",
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesServicesValue {
        ///The entrypoint to bind to.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub entrypoint: ::std::option::Option<::std::string::String>,
        ///The Service environment.
        pub environment: ::std::string::String,
        ///The Service name.
        pub service: ::std::string::String,
    }

    ///The usage model for Pages Functions.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The usage model for Pages Functions.",
    ///  "deprecated": true,
    ///  "examples": [
    ///    "standard"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "standard",
    ///    "bundled",
    ///    "unbound"
    ///  ],
    ///  "x-auditable": true,
    ///  "x-stainless-deprecation-message": "All new projects now use the
    /// Standard usage model."
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
    pub enum PagesDeploymentConfigValuesUsageModel {
        #[serde(rename = "standard")]
        Standard,
        #[serde(rename = "bundled")]
        Bundled,
        #[serde(rename = "unbound")]
        Unbound,
    }

    impl ::std::fmt::Display for PagesDeploymentConfigValuesUsageModel {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Standard => f.write_str("standard"),
                Self::Bundled => f.write_str("bundled"),
                Self::Unbound => f.write_str("unbound"),
            }
        }
    }

    impl ::std::str::FromStr for PagesDeploymentConfigValuesUsageModel {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "standard" => Ok(Self::Standard),
                "bundled" => Ok(Self::Bundled),
                "unbound" => Ok(Self::Unbound),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesDeploymentConfigValuesUsageModel {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesDeploymentConfigValuesUsageModel {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesDeploymentConfigValuesUsageModel {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Vectorize binding.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Vectorize binding.",
    ///  "type": "object",
    ///  "required": [
    ///    "index_name"
    ///  ],
    ///  "properties": {
    ///    "index_name": {
    ///      "examples": [
    ///        "my_index"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentConfigValuesVectorizeBindingsValue {
        pub index_name: ::std::string::String,
    }

    ///Info about what caused the deployment.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Info about what caused the deployment.",
    ///  "readOnly": true,
    ///  "type": "object",
    ///  "required": [
    ///    "metadata",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "metadata": {
    ///      "description": "Additional info about the trigger.",
    ///      "type": "object",
    ///      "required": [
    ///        "branch",
    ///        "commit_dirty",
    ///        "commit_hash",
    ///        "commit_message"
    ///      ],
    ///      "properties": {
    ///        "branch": {
    ///          "description": "Where the trigger happened.",
    ///          "examples": [
    ///            "main"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "commit_dirty": {
    ///          "description": "Whether the deployment trigger commit was
    /// dirty.",
    ///          "examples": [
    ///            false
    ///          ],
    ///          "type": "boolean",
    ///          "x-auditable": true
    ///        },
    ///        "commit_hash": {
    ///          "description": "Hash of the deployment trigger commit.",
    ///          "examples": [
    ///            "ad9ccd918a81025731e10e40267e11273a263421"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "commit_message": {
    ///          "description": "Message of the deployment trigger commit.",
    ///          "examples": [
    ///            "Update index.html"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    },
    ///    "type": {
    ///      "description": "What caused the deployment.",
    ///      "examples": [
    ///        "ad_hoc"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "github:push",
    ///        "ad_hoc",
    ///        "deploy_hook"
    ///      ],
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentDeploymentTrigger {
        pub metadata: PagesDeploymentDeploymentTriggerMetadata,
        ///What caused the deployment.
        #[serde(rename = "type")]
        pub type_: PagesDeploymentDeploymentTriggerType,
    }

    ///Additional info about the trigger.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Additional info about the trigger.",
    ///  "type": "object",
    ///  "required": [
    ///    "branch",
    ///    "commit_dirty",
    ///    "commit_hash",
    ///    "commit_message"
    ///  ],
    ///  "properties": {
    ///    "branch": {
    ///      "description": "Where the trigger happened.",
    ///      "examples": [
    ///        "main"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "commit_dirty": {
    ///      "description": "Whether the deployment trigger commit was dirty.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "commit_hash": {
    ///      "description": "Hash of the deployment trigger commit.",
    ///      "examples": [
    ///        "ad9ccd918a81025731e10e40267e11273a263421"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "commit_message": {
    ///      "description": "Message of the deployment trigger commit.",
    ///      "examples": [
    ///        "Update index.html"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDeploymentDeploymentTriggerMetadata {
        ///Where the trigger happened.
        pub branch: ::std::string::String,
        ///Whether the deployment trigger commit was dirty.
        pub commit_dirty: bool,
        ///Hash of the deployment trigger commit.
        pub commit_hash: ::std::string::String,
        ///Message of the deployment trigger commit.
        pub commit_message: ::std::string::String,
    }

    ///What caused the deployment.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "What caused the deployment.",
    ///  "examples": [
    ///    "ad_hoc"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "github:push",
    ///    "ad_hoc",
    ///    "deploy_hook"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesDeploymentDeploymentTriggerType {
        #[serde(rename = "github:push")]
        GithubPush,
        #[serde(rename = "ad_hoc")]
        AdHoc,
        #[serde(rename = "deploy_hook")]
        DeployHook,
    }

    impl ::std::fmt::Display for PagesDeploymentDeploymentTriggerType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::GithubPush => f.write_str("github:push"),
                Self::AdHoc => f.write_str("ad_hoc"),
                Self::DeployHook => f.write_str("deploy_hook"),
            }
        }
    }

    impl ::std::str::FromStr for PagesDeploymentDeploymentTriggerType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "github:push" => Ok(Self::GithubPush),
                "ad_hoc" => Ok(Self::AdHoc),
                "deploy_hook" => Ok(Self::DeployHook),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesDeploymentDeploymentTriggerType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesDeploymentDeploymentTriggerType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesDeploymentDeploymentTriggerType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Type of deploy.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Type of deploy.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "preview"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "preview",
    ///    "production"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesDeploymentEnvironment {
        #[serde(rename = "preview")]
        Preview,
        #[serde(rename = "production")]
        Production,
    }

    impl ::std::fmt::Display for PagesDeploymentEnvironment {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Preview => f.write_str("preview"),
                Self::Production => f.write_str("production"),
            }
        }
    }

    impl ::std::str::FromStr for PagesDeploymentEnvironment {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "preview" => Ok(Self::Preview),
                "production" => Ok(Self::Production),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesDeploymentEnvironment {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesDeploymentEnvironment {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesDeploymentEnvironment {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`PagesDomain`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "certificate_authority",
    ///    "created_on",
    ///    "domain_id",
    ///    "id",
    ///    "name",
    ///    "status",
    ///    "validation_data",
    ///    "verification_data",
    ///    "zone_tag"
    ///  ],
    ///  "properties": {
    ///    "certificate_authority": {
    ///      "readOnly": true,
    ///      "examples": [
    ///        "lets_encrypt"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "google",
    ///        "lets_encrypt"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "created_on": {
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "domain_id": {
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "id": {
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "name": {
    ///      "$ref": "#/components/schemas/pages_domain_name"
    ///    },
    ///    "status": {
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "enum": [
    ///        "initializing",
    ///        "pending",
    ///        "active",
    ///        "deactivated",
    ///        "blocked",
    ///        "error"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "validation_data": {
    ///      "readOnly": true,
    ///      "type": "object",
    ///      "required": [
    ///        "method",
    ///        "status"
    ///      ],
    ///      "properties": {
    ///        "error_message": {
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "method": {
    ///          "type": "string",
    ///          "enum": [
    ///            "http",
    ///            "txt"
    ///          ],
    ///          "x-auditable": true
    ///        },
    ///        "status": {
    ///          "type": "string",
    ///          "enum": [
    ///            "initializing",
    ///            "pending",
    ///            "active",
    ///            "deactivated",
    ///            "error"
    ///          ],
    ///          "x-auditable": true
    ///        },
    ///        "txt_name": {
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "txt_value": {
    ///          "type": "string",
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    },
    ///    "verification_data": {
    ///      "readOnly": true,
    ///      "type": "object",
    ///      "required": [
    ///        "status"
    ///      ],
    ///      "properties": {
    ///        "error_message": {
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "status": {
    ///          "type": "string",
    ///          "enum": [
    ///            "pending",
    ///            "active",
    ///            "deactivated",
    ///            "blocked",
    ///            "error"
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    "zone_tag": {
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDomain {
        pub certificate_authority: PagesDomainCertificateAuthority,
        pub created_on: ::std::string::String,
        pub domain_id: ::std::string::String,
        pub id: ::std::string::String,
        pub name: PagesDomainName,
        pub status: PagesDomainStatus,
        pub validation_data: PagesDomainValidationData,
        pub verification_data: PagesDomainVerificationData,
        pub zone_tag: ::std::string::String,
    }

    ///`PagesDomainCertificateAuthority`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "readOnly": true,
    ///  "examples": [
    ///    "lets_encrypt"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "google",
    ///    "lets_encrypt"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesDomainCertificateAuthority {
        #[serde(rename = "google")]
        Google,
        #[serde(rename = "lets_encrypt")]
        LetsEncrypt,
    }

    impl ::std::fmt::Display for PagesDomainCertificateAuthority {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Google => f.write_str("google"),
                Self::LetsEncrypt => f.write_str("lets_encrypt"),
            }
        }
    }

    impl ::std::str::FromStr for PagesDomainCertificateAuthority {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "google" => Ok(Self::Google),
                "lets_encrypt" => Ok(Self::LetsEncrypt),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesDomainCertificateAuthority {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesDomainCertificateAuthority {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesDomainCertificateAuthority {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///The domain name.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The domain name.",
    ///  "examples": [
    ///    "this-is-my-domain-01.com"
    ///  ],
    ///  "type": "string",
    ///  "pattern": "^([a-zA-Z0-9][\\-a-zA-Z0-9]*\\.)+[\\-a-zA-Z0-9]{2,20}$",
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct PagesDomainName(::std::string::String);
    impl ::std::ops::Deref for PagesDomainName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<PagesDomainName> for ::std::string::String {
        fn from(value: PagesDomainName) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for PagesDomainName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^([a-zA-Z0-9][\\-a-zA-Z0-9]*\\.)+[\\-a-zA-Z0-9]{2,20}$")
                        .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^([a-zA-Z0-9][\\-a-zA-Z0-9]*\\.)+[\\-a-zA-Z0-9]{2,20}$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesDomainName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesDomainName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesDomainName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for PagesDomainName {
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

    ///`PagesDomainStatus`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "readOnly": true,
    ///  "type": "string",
    ///  "enum": [
    ///    "initializing",
    ///    "pending",
    ///    "active",
    ///    "deactivated",
    ///    "blocked",
    ///    "error"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesDomainStatus {
        #[serde(rename = "initializing")]
        Initializing,
        #[serde(rename = "pending")]
        Pending,
        #[serde(rename = "active")]
        Active,
        #[serde(rename = "deactivated")]
        Deactivated,
        #[serde(rename = "blocked")]
        Blocked,
        #[serde(rename = "error")]
        Error,
    }

    impl ::std::fmt::Display for PagesDomainStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Initializing => f.write_str("initializing"),
                Self::Pending => f.write_str("pending"),
                Self::Active => f.write_str("active"),
                Self::Deactivated => f.write_str("deactivated"),
                Self::Blocked => f.write_str("blocked"),
                Self::Error => f.write_str("error"),
            }
        }
    }

    impl ::std::str::FromStr for PagesDomainStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "initializing" => Ok(Self::Initializing),
                "pending" => Ok(Self::Pending),
                "active" => Ok(Self::Active),
                "deactivated" => Ok(Self::Deactivated),
                "blocked" => Ok(Self::Blocked),
                "error" => Ok(Self::Error),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesDomainStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesDomainStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesDomainStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`PagesDomainValidationData`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "readOnly": true,
    ///  "type": "object",
    ///  "required": [
    ///    "method",
    ///    "status"
    ///  ],
    ///  "properties": {
    ///    "error_message": {
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "method": {
    ///      "type": "string",
    ///      "enum": [
    ///        "http",
    ///        "txt"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "status": {
    ///      "type": "string",
    ///      "enum": [
    ///        "initializing",
    ///        "pending",
    ///        "active",
    ///        "deactivated",
    ///        "error"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "txt_name": {
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "txt_value": {
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDomainValidationData {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error_message: ::std::option::Option<::std::string::String>,
        pub method: PagesDomainValidationDataMethod,
        pub status: PagesDomainValidationDataStatus,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub txt_name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub txt_value: ::std::option::Option<::std::string::String>,
    }

    ///`PagesDomainValidationDataMethod`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "http",
    ///    "txt"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesDomainValidationDataMethod {
        #[serde(rename = "http")]
        Http,
        #[serde(rename = "txt")]
        Txt,
    }

    impl ::std::fmt::Display for PagesDomainValidationDataMethod {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Http => f.write_str("http"),
                Self::Txt => f.write_str("txt"),
            }
        }
    }

    impl ::std::str::FromStr for PagesDomainValidationDataMethod {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "http" => Ok(Self::Http),
                "txt" => Ok(Self::Txt),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesDomainValidationDataMethod {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesDomainValidationDataMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesDomainValidationDataMethod {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`PagesDomainValidationDataStatus`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "initializing",
    ///    "pending",
    ///    "active",
    ///    "deactivated",
    ///    "error"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesDomainValidationDataStatus {
        #[serde(rename = "initializing")]
        Initializing,
        #[serde(rename = "pending")]
        Pending,
        #[serde(rename = "active")]
        Active,
        #[serde(rename = "deactivated")]
        Deactivated,
        #[serde(rename = "error")]
        Error,
    }

    impl ::std::fmt::Display for PagesDomainValidationDataStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Initializing => f.write_str("initializing"),
                Self::Pending => f.write_str("pending"),
                Self::Active => f.write_str("active"),
                Self::Deactivated => f.write_str("deactivated"),
                Self::Error => f.write_str("error"),
            }
        }
    }

    impl ::std::str::FromStr for PagesDomainValidationDataStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "initializing" => Ok(Self::Initializing),
                "pending" => Ok(Self::Pending),
                "active" => Ok(Self::Active),
                "deactivated" => Ok(Self::Deactivated),
                "error" => Ok(Self::Error),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesDomainValidationDataStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesDomainValidationDataStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesDomainValidationDataStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`PagesDomainVerificationData`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "readOnly": true,
    ///  "type": "object",
    ///  "required": [
    ///    "status"
    ///  ],
    ///  "properties": {
    ///    "error_message": {
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "status": {
    ///      "type": "string",
    ///      "enum": [
    ///        "pending",
    ///        "active",
    ///        "deactivated",
    ///        "blocked",
    ///        "error"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDomainVerificationData {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error_message: ::std::option::Option<::std::string::String>,
        pub status: PagesDomainVerificationDataStatus,
    }

    ///`PagesDomainVerificationDataStatus`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "pending",
    ///    "active",
    ///    "deactivated",
    ///    "blocked",
    ///    "error"
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
    pub enum PagesDomainVerificationDataStatus {
        #[serde(rename = "pending")]
        Pending,
        #[serde(rename = "active")]
        Active,
        #[serde(rename = "deactivated")]
        Deactivated,
        #[serde(rename = "blocked")]
        Blocked,
        #[serde(rename = "error")]
        Error,
    }

    impl ::std::fmt::Display for PagesDomainVerificationDataStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Pending => f.write_str("pending"),
                Self::Active => f.write_str("active"),
                Self::Deactivated => f.write_str("deactivated"),
                Self::Blocked => f.write_str("blocked"),
                Self::Error => f.write_str("error"),
            }
        }
    }

    impl ::std::str::FromStr for PagesDomainVerificationDataStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "pending" => Ok(Self::Pending),
                "active" => Ok(Self::Active),
                "deactivated" => Ok(Self::Deactivated),
                "blocked" => Ok(Self::Blocked),
                "error" => Ok(Self::Error),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesDomainVerificationDataStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesDomainVerificationDataStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesDomainVerificationDataStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`PagesDomainsAddDomainBody`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "name": {
    ///      "$ref": "#/components/schemas/pages_domain_name"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDomainsAddDomainBody {
        pub name: PagesDomainName,
    }

    ///`PagesDomainsAddDomainResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/pages_domain"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDomainsAddDomainResponse {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        pub result: PagesDomain,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`PagesDomainsDeleteDomainResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "type": [
    ///            "object",
    ///            "null"
    ///          ]
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDomainsDeleteDomainResponse {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        pub result:
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`PagesDomainsGetDomainResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/pages_domain"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDomainsGetDomainResponse {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        pub result: PagesDomain,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`PagesDomainsGetDomainsResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_api-response-collection"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "type": "array",
    ///          "items": {
    ///            "$ref": "#/components/schemas/pages_domain"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDomainsGetDomainsResponse {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        pub result: ::std::vec::Vec<PagesDomain>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result_info: ::std::option::Option<PagesDomainsGetDomainsResponseResultInfo>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`PagesDomainsGetDomainsResponseResultInfo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "count": {
    ///      "description": "Total number of results for the requested
    /// service.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "page": {
    ///      "description": "Current page within paginated list of results.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "per_page": {
    ///      "description": "Number of results per page of results.",
    ///      "examples": [
    ///        20
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_count": {
    ///      "description": "Total results available without any search
    /// parameters.",
    ///      "examples": [
    ///        2000
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_pages": {
    ///      "description": "The number of total pages in the entire result
    /// set.",
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDomainsGetDomainsResponseResultInfo {
        ///Total number of results for the requested service.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub count: ::std::option::Option<f64>,
        ///Current page within paginated list of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub page: ::std::option::Option<f64>,
        ///Number of results per page of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub per_page: ::std::option::Option<f64>,
        ///Total results available without any search parameters.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_count: ::std::option::Option<f64>,
        ///The number of total pages in the entire result set.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_pages: ::std::option::Option<f64>,
    }

    impl ::std::default::Default for PagesDomainsGetDomainsResponseResultInfo {
        fn default() -> Self {
            Self {
                count: Default::default(),
                page: Default::default(),
                per_page: Default::default(),
                total_count: Default::default(),
                total_pages: Default::default(),
            }
        }
    }

    ///`PagesDomainsPatchDomainResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/pages_domain"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesDomainsPatchDomainResponse {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        pub result: PagesDomain,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///Environment variables used for builds and Pages Functions.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Environment variables used for builds and Pages
    /// Functions.",
    ///  "type": [
    ///    "object",
    ///    "null"
    ///  ],
    ///  "additionalProperties": {
    ///    "type": [
    ///      "object",
    ///      "null"
    ///    ],
    ///    "oneOf": [
    ///      {
    ///        "$ref": "#/components/schemas/pages_plain_text_env_var"
    ///      },
    ///      {
    ///        "$ref": "#/components/schemas/pages_secret_text_env_var"
    ///      }
    ///    ]
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct PagesEnvVars(
        pub  ::std::option::Option<
            ::std::collections::HashMap<
                ::std::string::String,
                ::std::option::Option<PagesEnvVarsInnerValue>,
            >,
        >,
    );
    impl ::std::ops::Deref for PagesEnvVars {
        type Target = ::std::option::Option<
            ::std::collections::HashMap<
                ::std::string::String,
                ::std::option::Option<PagesEnvVarsInnerValue>,
            >,
        >;
        fn deref(
            &self,
        ) -> &::std::option::Option<
            ::std::collections::HashMap<
                ::std::string::String,
                ::std::option::Option<PagesEnvVarsInnerValue>,
            >,
        > {
            &self.0
        }
    }

    impl ::std::convert::From<PagesEnvVars>
        for ::std::option::Option<
            ::std::collections::HashMap<
                ::std::string::String,
                ::std::option::Option<PagesEnvVarsInnerValue>,
            >,
        >
    {
        fn from(value: PagesEnvVars) -> Self {
            value.0
        }
    }

    impl
        ::std::convert::From<
            ::std::option::Option<
                ::std::collections::HashMap<
                    ::std::string::String,
                    ::std::option::Option<PagesEnvVarsInnerValue>,
                >,
            >,
        > for PagesEnvVars
    {
        fn from(
            value: ::std::option::Option<
                ::std::collections::HashMap<
                    ::std::string::String,
                    ::std::option::Option<PagesEnvVarsInnerValue>,
                >,
            >,
        ) -> Self {
            Self(value)
        }
    }

    ///`PagesEnvVarsInnerValue`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "oneOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_plain_text_env_var"
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/pages_secret_text_env_var"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum PagesEnvVarsInnerValue {
        PlainTextEnvVar(PagesPlainTextEnvVar),
        SecretTextEnvVar(PagesSecretTextEnvVar),
    }

    impl ::std::convert::From<PagesPlainTextEnvVar> for PagesEnvVarsInnerValue {
        fn from(value: PagesPlainTextEnvVar) -> Self {
            Self::PlainTextEnvVar(value)
        }
    }

    impl ::std::convert::From<PagesSecretTextEnvVar> for PagesEnvVarsInnerValue {
        fn from(value: PagesSecretTextEnvVar) -> Self {
            Self::SecretTextEnvVar(value)
        }
    }

    ///Identifier.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Identifier.",
    ///  "examples": [
    ///    "023e105f4ecef8ad9ca31a8372d0c353"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32,
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct PagesIdentifier(::std::string::String);
    impl ::std::ops::Deref for PagesIdentifier {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<PagesIdentifier> for ::std::string::String {
        fn from(value: PagesIdentifier) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for PagesIdentifier {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for PagesIdentifier {
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

    ///`PagesMessages`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    []
    ///  ],
    ///  "type": "array",
    ///  "items": {
    ///    "type": "object",
    ///    "uniqueItems": true,
    ///    "required": [
    ///      "code",
    ///      "message"
    ///    ],
    ///    "properties": {
    ///      "code": {
    ///        "type": "integer",
    ///        "minimum": 1000.0
    ///      },
    ///      "documentation_url": {
    ///        "type": "string"
    ///      },
    ///      "message": {
    ///        "type": "string"
    ///      },
    ///      "source": {
    ///        "type": "object",
    ///        "properties": {
    ///          "pointer": {
    ///            "type": "string"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct PagesMessages(pub ::std::vec::Vec<PagesMessagesItem>);
    impl ::std::ops::Deref for PagesMessages {
        type Target = ::std::vec::Vec<PagesMessagesItem>;
        fn deref(&self) -> &::std::vec::Vec<PagesMessagesItem> {
            &self.0
        }
    }

    impl ::std::convert::From<PagesMessages> for ::std::vec::Vec<PagesMessagesItem> {
        fn from(value: PagesMessages) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::vec::Vec<PagesMessagesItem>> for PagesMessages {
        fn from(value: ::std::vec::Vec<PagesMessagesItem>) -> Self {
            Self(value)
        }
    }

    ///`PagesMessagesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "documentation_url": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "source": {
    ///      "type": "object",
    ///      "properties": {
    ///        "pointer": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesMessagesItem {
        pub code: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub documentation_url: ::std::option::Option<::std::string::String>,
        pub message: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<PagesMessagesItemSource>,
    }

    ///`PagesMessagesItemSource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "pointer": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesMessagesItemSource {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pointer: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for PagesMessagesItemSource {
        fn default() -> Self {
            Self {
                pointer: Default::default(),
            }
        }
    }

    ///A plaintext environment variable.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A plaintext environment variable.",
    ///  "examples": [
    ///    {
    ///      "type": "plain_text",
    ///      "value": "hello world"
    ///    }
    ///  ],
    ///  "type": [
    ///    "object",
    ///    "null"
    ///  ],
    ///  "required": [
    ///    "type",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "plain_text"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "value": {
    ///      "description": "Environment variable value.",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct PagesPlainTextEnvVar(pub ::std::option::Option<PagesPlainTextEnvVarInner>);
    impl ::std::ops::Deref for PagesPlainTextEnvVar {
        type Target = ::std::option::Option<PagesPlainTextEnvVarInner>;
        fn deref(&self) -> &::std::option::Option<PagesPlainTextEnvVarInner> {
            &self.0
        }
    }

    impl ::std::convert::From<PagesPlainTextEnvVar>
        for ::std::option::Option<PagesPlainTextEnvVarInner>
    {
        fn from(value: PagesPlainTextEnvVar) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<PagesPlainTextEnvVarInner>>
        for PagesPlainTextEnvVar
    {
        fn from(value: ::std::option::Option<PagesPlainTextEnvVarInner>) -> Self {
            Self(value)
        }
    }

    ///A plaintext environment variable.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A plaintext environment variable.",
    ///  "examples": [
    ///    {
    ///      "type": "plain_text",
    ///      "value": "hello world"
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "required": [
    ///    "type",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "plain_text"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "value": {
    ///      "description": "Environment variable value.",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesPlainTextEnvVarInner {
        #[serde(rename = "type")]
        pub type_: PagesPlainTextEnvVarInnerType,
        ///Environment variable value.
        pub value: ::std::string::String,
    }

    ///`PagesPlainTextEnvVarInnerType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "plain_text"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesPlainTextEnvVarInnerType {
        #[serde(rename = "plain_text")]
        PlainText,
    }

    impl ::std::fmt::Display for PagesPlainTextEnvVarInnerType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::PlainText => f.write_str("plain_text"),
            }
        }
    }

    impl ::std::str::FromStr for PagesPlainTextEnvVarInnerType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "plain_text" => Ok(Self::PlainText),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesPlainTextEnvVarInnerType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesPlainTextEnvVarInnerType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesPlainTextEnvVarInnerType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`PagesProject`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "canonical_deployment",
    ///    "created_on",
    ///    "deployment_configs",
    ///    "framework",
    ///    "framework_version",
    ///    "id",
    ///    "latest_deployment",
    ///    "name",
    ///    "preview_script_name",
    ///    "production_branch",
    ///    "production_script_name",
    ///    "uses_functions"
    ///  ],
    ///  "properties": {
    ///    "build_config": {
    ///      "$ref": "#/components/schemas/pages_build_config"
    ///    },
    ///    "canonical_deployment": {
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/pages_deployment"
    ///        },
    ///        {
    ///          "description": "Most recent production deployment of the
    /// project.",
    ///          "readOnly": true,
    ///          "type": [
    ///            "object",
    ///            "null"
    ///          ]
    ///        }
    ///      ]
    ///    },
    ///    "created_on": {
    ///      "description": "When the project was created.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "2017-01-01T00:00:00Z"
    ///      ],
    ///      "type": "string",
    ///      "format": "date-time",
    ///      "x-auditable": true
    ///    },
    ///    "deployment_configs": {
    ///      "description": "Configs for deployments in a project.",
    ///      "type": "object",
    ///      "required": [
    ///        "preview",
    ///        "production"
    ///      ],
    ///      "properties": {
    ///        "preview": {
    ///          "description": "Configs for preview deploys.",
    ///          "allOf": [
    ///            {
    ///              "$ref":
    /// "#/components/schemas/pages_deployment_config_values"
    ///            }
    ///          ]
    ///        },
    ///        "production": {
    ///          "description": "Configs for production deploys.",
    ///          "allOf": [
    ///            {
    ///              "$ref":
    /// "#/components/schemas/pages_deployment_config_values"
    ///            }
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    "domains": {
    ///      "description": "A list of associated custom domains for the
    /// project.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        [
    ///          "customdomain.com",
    ///          "customdomain.org"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "framework": {
    ///      "description": "Framework the project is using.",
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "framework_version": {
    ///      "description": "Version of the framework the project is using.",
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "id": {
    ///      "description": "ID of the project.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "7b162ea7-7367-4d67-bcde-1160995d5"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "latest_deployment": {
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/pages_deployment"
    ///        },
    ///        {
    ///          "description": "Most recent deployment of the project.",
    ///          "readOnly": true,
    ///          "type": [
    ///            "object",
    ///            "null"
    ///          ]
    ///        }
    ///      ]
    ///    },
    ///    "name": {
    ///      "$ref": "#/components/schemas/pages_project_name"
    ///    },
    ///    "preview_script_name": {
    ///      "description": "Name of the preview script.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "pages-worker--1234567-preview"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "production_branch": {
    ///      "description": "Production branch of the project. Used to identify
    /// production deployments.",
    ///      "examples": [
    ///        "main"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "production_script_name": {
    ///      "description": "Name of the production script.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "pages-worker--1234567-production"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "source": {
    ///      "$ref": "#/components/schemas/pages_source"
    ///    },
    ///    "subdomain": {
    ///      "description": "The Cloudflare subdomain associated with the
    /// project.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "helloworld.pages.dev"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "uses_functions": {
    ///      "description": "Whether the project uses functions.",
    ///      "readOnly": true,
    ///      "type": [
    ///        "boolean",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProject {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub build_config: ::std::option::Option<PagesBuildConfig>,
        pub canonical_deployment: PagesDeployment,
        ///When the project was created.
        pub created_on: ::chrono::DateTime<::chrono::offset::Utc>,
        pub deployment_configs: PagesProjectDeploymentConfigs,
        ///A list of associated custom domains for the project.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub domains: ::std::vec::Vec<::std::string::String>,
        ///Framework the project is using.
        pub framework: ::std::string::String,
        ///Version of the framework the project is using.
        pub framework_version: ::std::string::String,
        ///ID of the project.
        pub id: ::std::string::String,
        pub latest_deployment: PagesDeployment,
        pub name: PagesProjectName,
        ///Name of the preview script.
        pub preview_script_name: ::std::string::String,
        ///Production branch of the project. Used to identify production
        /// deployments.
        pub production_branch: ::std::string::String,
        ///Name of the production script.
        pub production_script_name: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<PagesSource>,
        ///The Cloudflare subdomain associated with the project.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub subdomain: ::std::option::Option<::std::string::String>,
        ///Whether the project uses functions.
        pub uses_functions: ::std::option::Option<bool>,
    }

    ///`PagesProjectCreateProjectBody`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "name",
    ///    "production_branch"
    ///  ],
    ///  "properties": {
    ///    "build_config": {
    ///      "description": "Configs for the project build process.",
    ///      "type": "object",
    ///      "properties": {
    ///        "build_caching": {
    ///          "description": "Enable build caching for the project.",
    ///          "examples": [
    ///            true
    ///          ],
    ///          "type": "boolean",
    ///          "x-auditable": true
    ///        },
    ///        "build_command": {
    ///          "description": "Command used to build project.",
    ///          "examples": [
    ///            "npm run build"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "destination_dir": {
    ///          "description": "Output directory of the build.",
    ///          "examples": [
    ///            "build"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "root_dir": {
    ///          "description": "Directory to run the command.",
    ///          "examples": [
    ///            "/"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "web_analytics_tag": {
    ///          "description": "The classifying tag for analytics.",
    ///          "examples": [
    ///            "cee1c73f6e4743d0b5e6bb1a0bcaabcc"
    ///          ],
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ],
    ///          "x-auditable": true
    ///        },
    ///        "web_analytics_token": {
    ///          "description": "The auth token for analytics.",
    ///          "examples": [
    ///            "021e1057c18547eca7b79f2516f06o7x"
    ///          ],
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ],
    ///          "x-sensitive": true
    ///        }
    ///      }
    ///    },
    ///    "deployment_configs": {
    ///      "description": "Configs for deployments in a project.",
    ///      "type": "object",
    ///      "properties": {
    ///        "preview": {
    ///          "description": "Configs for preview deploys.",
    ///          "allOf": [
    ///            {
    ///              "$ref":
    /// "#/components/schemas/pages_deployment_config_values_request"
    ///            }
    ///          ]
    ///        },
    ///        "production": {
    ///          "description": "Configs for production deploys.",
    ///          "allOf": [
    ///            {
    ///              "$ref":
    /// "#/components/schemas/pages_deployment_config_values_request"
    ///            }
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    "name": {
    ///      "description": "Name of the project.",
    ///      "examples": [
    ///        "my-pages-app"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "production_branch": {
    ///      "description": "Production branch of the project. Used to identify
    /// production deployments.",
    ///      "examples": [
    ///        "main"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "source": {
    ///      "description": "Configs for the project source control.",
    ///      "type": "object",
    ///      "required": [
    ///        "config",
    ///        "type"
    ///      ],
    ///      "properties": {
    ///        "config": {
    ///          "type": "object",
    ///          "properties": {
    ///            "deployments_enabled": {
    ///              "description": "Whether to enable automatic deployments
    /// when pushing to the source repository.\nWhen disabled, no deployments
    /// (production or preview) will be triggered automatically.\n",
    ///              "deprecated": true,
    ///              "type": "boolean",
    ///              "x-auditable": true,
    ///              "x-stainless-deprecation-message": "Use
    /// `production_deployments_enabled` and `preview_deployment_setting` for
    /// more granular control."
    ///            },
    ///            "owner": {
    ///              "description": "The owner of the repository.",
    ///              "examples": [
    ///                "my-org"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            },
    ///            "owner_id": {
    ///              "description": "The owner ID of the repository.",
    ///              "examples": [
    ///                "12345678"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            },
    ///            "path_excludes": {
    ///              "description": "A list of paths that should be excluded
    /// from triggering a preview deployment. Wildcard syntax (`*`) is
    /// supported.",
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string",
    ///                "x-auditable": true
    ///              }
    ///            },
    ///            "path_includes": {
    ///              "description": "A list of paths that should be watched to
    /// trigger a preview deployment. Wildcard syntax (`*`) is supported.",
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string",
    ///                "x-auditable": true
    ///              }
    ///            },
    ///            "pr_comments_enabled": {
    ///              "description": "Whether to enable PR comments.",
    ///              "type": "boolean",
    ///              "x-auditable": true
    ///            },
    ///            "preview_branch_excludes": {
    ///              "description": "A list of branches that should not trigger
    /// a preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string",
    ///                "x-auditable": true
    ///              }
    ///            },
    ///            "preview_branch_includes": {
    ///              "description": "A list of branches that should trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string",
    ///                "x-auditable": true
    ///              }
    ///            },
    ///            "preview_deployment_setting": {
    ///              "description": "Controls whether commits to preview
    /// branches trigger a preview deployment.",
    ///              "type": "string",
    ///              "enum": [
    ///                "all",
    ///                "none",
    ///                "custom"
    ///              ],
    ///              "x-auditable": true
    ///            },
    ///            "production_branch": {
    ///              "description": "The production branch of the repository.",
    ///              "examples": [
    ///                "main"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            },
    ///            "production_deployments_enabled": {
    ///              "description": "Whether to trigger a production deployment
    /// on commits to the production branch.",
    ///              "type": "boolean",
    ///              "x-auditable": true
    ///            },
    ///            "repo_id": {
    ///              "description": "The ID of the repository.",
    ///              "examples": [
    ///                "12345678"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            },
    ///            "repo_name": {
    ///              "description": "The name of the repository.",
    ///              "examples": [
    ///                "my-repo"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            }
    ///          }
    ///        },
    ///        "type": {
    ///          "description": "The source control management provider.",
    ///          "examples": [
    ///            "github"
    ///          ],
    ///          "type": "string",
    ///          "enum": [
    ///            "github",
    ///            "gitlab"
    ///          ],
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectCreateProjectBody {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub build_config: ::std::option::Option<PagesProjectCreateProjectBodyBuildConfig>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub deployment_configs:
            ::std::option::Option<PagesProjectCreateProjectBodyDeploymentConfigs>,
        ///Name of the project.
        pub name: ::std::string::String,
        ///Production branch of the project. Used to identify production
        /// deployments.
        pub production_branch: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<PagesProjectCreateProjectBodySource>,
    }

    ///Configs for the project build process.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Configs for the project build process.",
    ///  "type": "object",
    ///  "properties": {
    ///    "build_caching": {
    ///      "description": "Enable build caching for the project.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "build_command": {
    ///      "description": "Command used to build project.",
    ///      "examples": [
    ///        "npm run build"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "destination_dir": {
    ///      "description": "Output directory of the build.",
    ///      "examples": [
    ///        "build"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "root_dir": {
    ///      "description": "Directory to run the command.",
    ///      "examples": [
    ///        "/"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "web_analytics_tag": {
    ///      "description": "The classifying tag for analytics.",
    ///      "examples": [
    ///        "cee1c73f6e4743d0b5e6bb1a0bcaabcc"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "web_analytics_token": {
    ///      "description": "The auth token for analytics.",
    ///      "examples": [
    ///        "021e1057c18547eca7b79f2516f06o7x"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-sensitive": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectCreateProjectBodyBuildConfig {
        ///Enable build caching for the project.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub build_caching: ::std::option::Option<bool>,
        ///Command used to build project.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub build_command: ::std::option::Option<::std::string::String>,
        ///Output directory of the build.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub destination_dir: ::std::option::Option<::std::string::String>,
        ///Directory to run the command.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub root_dir: ::std::option::Option<::std::string::String>,
        ///The classifying tag for analytics.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub web_analytics_tag: ::std::option::Option<::std::string::String>,
        ///The auth token for analytics.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub web_analytics_token: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for PagesProjectCreateProjectBodyBuildConfig {
        fn default() -> Self {
            Self {
                build_caching: Default::default(),
                build_command: Default::default(),
                destination_dir: Default::default(),
                root_dir: Default::default(),
                web_analytics_tag: Default::default(),
                web_analytics_token: Default::default(),
            }
        }
    }

    ///Configs for deployments in a project.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Configs for deployments in a project.",
    ///  "type": "object",
    ///  "properties": {
    ///    "preview": {
    ///      "description": "Configs for preview deploys.",
    ///      "allOf": [
    ///        {
    ///          "$ref":
    /// "#/components/schemas/pages_deployment_config_values_request"
    ///        }
    ///      ]
    ///    },
    ///    "production": {
    ///      "description": "Configs for production deploys.",
    ///      "allOf": [
    ///        {
    ///          "$ref":
    /// "#/components/schemas/pages_deployment_config_values_request"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectCreateProjectBodyDeploymentConfigs {
        ///Configs for preview deploys.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub preview: ::std::option::Option<PagesDeploymentConfigValuesRequest>,
        ///Configs for production deploys.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub production: ::std::option::Option<PagesDeploymentConfigValuesRequest>,
    }

    impl ::std::default::Default for PagesProjectCreateProjectBodyDeploymentConfigs {
        fn default() -> Self {
            Self {
                preview: Default::default(),
                production: Default::default(),
            }
        }
    }

    ///Configs for the project source control.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Configs for the project source control.",
    ///  "type": "object",
    ///  "required": [
    ///    "config",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "config": {
    ///      "type": "object",
    ///      "properties": {
    ///        "deployments_enabled": {
    ///          "description": "Whether to enable automatic deployments when
    /// pushing to the source repository.\nWhen disabled, no deployments
    /// (production or preview) will be triggered automatically.\n",
    ///          "deprecated": true,
    ///          "type": "boolean",
    ///          "x-auditable": true,
    ///          "x-stainless-deprecation-message": "Use
    /// `production_deployments_enabled` and `preview_deployment_setting` for
    /// more granular control."
    ///        },
    ///        "owner": {
    ///          "description": "The owner of the repository.",
    ///          "examples": [
    ///            "my-org"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "owner_id": {
    ///          "description": "The owner ID of the repository.",
    ///          "examples": [
    ///            "12345678"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "path_excludes": {
    ///          "description": "A list of paths that should be excluded from triggering a preview deployment. Wildcard syntax (`*`) is supported.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "path_includes": {
    ///          "description": "A list of paths that should be watched to
    /// trigger a preview deployment. Wildcard syntax (`*`) is supported.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "pr_comments_enabled": {
    ///          "description": "Whether to enable PR comments.",
    ///          "type": "boolean",
    ///          "x-auditable": true
    ///        },
    ///        "preview_branch_excludes": {
    ///          "description": "A list of branches that should not trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "preview_branch_includes": {
    ///          "description": "A list of branches that should trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "preview_deployment_setting": {
    ///          "description": "Controls whether commits to preview branches
    /// trigger a preview deployment.",
    ///          "type": "string",
    ///          "enum": [
    ///            "all",
    ///            "none",
    ///            "custom"
    ///          ],
    ///          "x-auditable": true
    ///        },
    ///        "production_branch": {
    ///          "description": "The production branch of the repository.",
    ///          "examples": [
    ///            "main"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "production_deployments_enabled": {
    ///          "description": "Whether to trigger a production deployment on
    /// commits to the production branch.",
    ///          "type": "boolean",
    ///          "x-auditable": true
    ///        },
    ///        "repo_id": {
    ///          "description": "The ID of the repository.",
    ///          "examples": [
    ///            "12345678"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "repo_name": {
    ///          "description": "The name of the repository.",
    ///          "examples": [
    ///            "my-repo"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    },
    ///    "type": {
    ///      "description": "The source control management provider.",
    ///      "examples": [
    ///        "github"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "github",
    ///        "gitlab"
    ///      ],
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectCreateProjectBodySource {
        pub config: PagesProjectCreateProjectBodySourceConfig,
        ///The source control management provider.
        #[serde(rename = "type")]
        pub type_: PagesProjectCreateProjectBodySourceType,
    }

    ///`PagesProjectCreateProjectBodySourceConfig`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "deployments_enabled": {
    ///      "description": "Whether to enable automatic deployments when
    /// pushing to the source repository.\nWhen disabled, no deployments
    /// (production or preview) will be triggered automatically.\n",
    ///      "deprecated": true,
    ///      "type": "boolean",
    ///      "x-auditable": true,
    ///      "x-stainless-deprecation-message": "Use
    /// `production_deployments_enabled` and `preview_deployment_setting` for
    /// more granular control."
    ///    },
    ///    "owner": {
    ///      "description": "The owner of the repository.",
    ///      "examples": [
    ///        "my-org"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "owner_id": {
    ///      "description": "The owner ID of the repository.",
    ///      "examples": [
    ///        "12345678"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "path_excludes": {
    ///      "description": "A list of paths that should be excluded from triggering a preview deployment. Wildcard syntax (`*`) is supported.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "path_includes": {
    ///      "description": "A list of paths that should be watched to trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "pr_comments_enabled": {
    ///      "description": "Whether to enable PR comments.",
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "preview_branch_excludes": {
    ///      "description": "A list of branches that should not trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "preview_branch_includes": {
    ///      "description": "A list of branches that should trigger a preview
    /// deployment. Wildcard syntax (`*`) is supported. Must be used with
    /// `preview_deployment_setting` set to `custom`.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "preview_deployment_setting": {
    ///      "description": "Controls whether commits to preview branches
    /// trigger a preview deployment.",
    ///      "type": "string",
    ///      "enum": [
    ///        "all",
    ///        "none",
    ///        "custom"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "production_branch": {
    ///      "description": "The production branch of the repository.",
    ///      "examples": [
    ///        "main"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "production_deployments_enabled": {
    ///      "description": "Whether to trigger a production deployment on
    /// commits to the production branch.",
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "repo_id": {
    ///      "description": "The ID of the repository.",
    ///      "examples": [
    ///        "12345678"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "repo_name": {
    ///      "description": "The name of the repository.",
    ///      "examples": [
    ///        "my-repo"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectCreateProjectBodySourceConfig {
        ///Whether to enable automatic deployments when pushing to the source
        /// repository. When disabled, no deployments (production or
        /// preview) will be triggered automatically.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub deployments_enabled: ::std::option::Option<bool>,
        ///The owner of the repository.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub owner: ::std::option::Option<::std::string::String>,
        ///The owner ID of the repository.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub owner_id: ::std::option::Option<::std::string::String>,
        ///A list of paths that should be excluded from triggering a preview
        /// deployment. Wildcard syntax (`*`) is supported.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub path_excludes: ::std::vec::Vec<::std::string::String>,
        ///A list of paths that should be watched to trigger a preview
        /// deployment. Wildcard syntax (`*`) is supported.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub path_includes: ::std::vec::Vec<::std::string::String>,
        ///Whether to enable PR comments.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pr_comments_enabled: ::std::option::Option<bool>,
        ///A list of branches that should not trigger a preview deployment.
        /// Wildcard syntax (`*`) is supported. Must be used with
        /// `preview_deployment_setting` set to `custom`.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub preview_branch_excludes: ::std::vec::Vec<::std::string::String>,
        ///A list of branches that should trigger a preview deployment.
        /// Wildcard syntax (`*`) is supported. Must be used with
        /// `preview_deployment_setting` set to `custom`.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub preview_branch_includes: ::std::vec::Vec<::std::string::String>,
        ///Controls whether commits to preview branches trigger a preview
        /// deployment.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub preview_deployment_setting: ::std::option::Option<
            PagesProjectCreateProjectBodySourceConfigPreviewDeploymentSetting,
        >,
        ///The production branch of the repository.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub production_branch: ::std::option::Option<::std::string::String>,
        ///Whether to trigger a production deployment on commits to the
        /// production branch.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub production_deployments_enabled: ::std::option::Option<bool>,
        ///The ID of the repository.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub repo_id: ::std::option::Option<::std::string::String>,
        ///The name of the repository.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub repo_name: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for PagesProjectCreateProjectBodySourceConfig {
        fn default() -> Self {
            Self {
                deployments_enabled: Default::default(),
                owner: Default::default(),
                owner_id: Default::default(),
                path_excludes: Default::default(),
                path_includes: Default::default(),
                pr_comments_enabled: Default::default(),
                preview_branch_excludes: Default::default(),
                preview_branch_includes: Default::default(),
                preview_deployment_setting: Default::default(),
                production_branch: Default::default(),
                production_deployments_enabled: Default::default(),
                repo_id: Default::default(),
                repo_name: Default::default(),
            }
        }
    }

    ///Controls whether commits to preview branches trigger a preview
    /// deployment.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Controls whether commits to preview branches trigger a
    /// preview deployment.",
    ///  "type": "string",
    ///  "enum": [
    ///    "all",
    ///    "none",
    ///    "custom"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesProjectCreateProjectBodySourceConfigPreviewDeploymentSetting {
        #[serde(rename = "all")]
        All,
        #[serde(rename = "none")]
        None,
        #[serde(rename = "custom")]
        Custom,
    }

    impl ::std::fmt::Display for PagesProjectCreateProjectBodySourceConfigPreviewDeploymentSetting {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::All => f.write_str("all"),
                Self::None => f.write_str("none"),
                Self::Custom => f.write_str("custom"),
            }
        }
    }

    impl ::std::str::FromStr for PagesProjectCreateProjectBodySourceConfigPreviewDeploymentSetting {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "all" => Ok(Self::All),
                "none" => Ok(Self::None),
                "custom" => Ok(Self::Custom),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str>
        for PagesProjectCreateProjectBodySourceConfigPreviewDeploymentSetting
    {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for PagesProjectCreateProjectBodySourceConfigPreviewDeploymentSetting
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for PagesProjectCreateProjectBodySourceConfigPreviewDeploymentSetting
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///The source control management provider.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The source control management provider.",
    ///  "examples": [
    ///    "github"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "github",
    ///    "gitlab"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesProjectCreateProjectBodySourceType {
        #[serde(rename = "github")]
        Github,
        #[serde(rename = "gitlab")]
        Gitlab,
    }

    impl ::std::fmt::Display for PagesProjectCreateProjectBodySourceType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Github => f.write_str("github"),
                Self::Gitlab => f.write_str("gitlab"),
            }
        }
    }

    impl ::std::str::FromStr for PagesProjectCreateProjectBodySourceType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "github" => Ok(Self::Github),
                "gitlab" => Ok(Self::Gitlab),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesProjectCreateProjectBodySourceType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesProjectCreateProjectBodySourceType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesProjectCreateProjectBodySourceType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`PagesProjectCreateProjectResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/pages_project"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectCreateProjectResponse {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        pub result: PagesProject,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`PagesProjectDeleteProjectResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "type": [
    ///            "object",
    ///            "null"
    ///          ]
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectDeleteProjectResponse {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        pub result:
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///Configs for deployments in a project.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Configs for deployments in a project.",
    ///  "type": "object",
    ///  "required": [
    ///    "preview",
    ///    "production"
    ///  ],
    ///  "properties": {
    ///    "preview": {
    ///      "description": "Configs for preview deploys.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/pages_deployment_config_values"
    ///        }
    ///      ]
    ///    },
    ///    "production": {
    ///      "description": "Configs for production deploys.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/pages_deployment_config_values"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectDeploymentConfigs {
        ///Configs for preview deploys.
        pub preview: PagesDeploymentConfigValues,
        ///Configs for production deploys.
        pub production: PagesDeploymentConfigValues,
    }

    ///`PagesProjectGetProjectResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/pages_project"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectGetProjectResponse {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        pub result: PagesProject,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`PagesProjectGetProjectsResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_api-response-collection"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "type": "array",
    ///          "items": {
    ///            "$ref": "#/components/schemas/pages_project"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectGetProjectsResponse {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        pub result: ::std::vec::Vec<PagesProject>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result_info: ::std::option::Option<PagesProjectGetProjectsResponseResultInfo>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`PagesProjectGetProjectsResponseResultInfo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "count": {
    ///      "description": "Total number of results for the requested
    /// service.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "page": {
    ///      "description": "Current page within paginated list of results.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "per_page": {
    ///      "description": "Number of results per page of results.",
    ///      "examples": [
    ///        20
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_count": {
    ///      "description": "Total results available without any search
    /// parameters.",
    ///      "examples": [
    ///        2000
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_pages": {
    ///      "description": "The number of total pages in the entire result
    /// set.",
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectGetProjectsResponseResultInfo {
        ///Total number of results for the requested service.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub count: ::std::option::Option<f64>,
        ///Current page within paginated list of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub page: ::std::option::Option<f64>,
        ///Number of results per page of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub per_page: ::std::option::Option<f64>,
        ///Total results available without any search parameters.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_count: ::std::option::Option<f64>,
        ///The number of total pages in the entire result set.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_pages: ::std::option::Option<f64>,
    }

    impl ::std::default::Default for PagesProjectGetProjectsResponseResultInfo {
        fn default() -> Self {
            Self {
                count: Default::default(),
                page: Default::default(),
                per_page: Default::default(),
                total_count: Default::default(),
                total_pages: Default::default(),
            }
        }
    }

    ///Name of the project.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Name of the project.",
    ///  "examples": [
    ///    "this-is-my-project-01"
    ///  ],
    ///  "type": "string",
    ///  "pattern": "^[a-z0-9][a-z0-9-]*$",
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct PagesProjectName(::std::string::String);
    impl ::std::ops::Deref for PagesProjectName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<PagesProjectName> for ::std::string::String {
        fn from(value: PagesProjectName) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for PagesProjectName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^[a-z0-9][a-z0-9-]*$").unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[a-z0-9][a-z0-9-]*$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesProjectName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesProjectName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesProjectName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for PagesProjectName {
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

    ///`PagesProjectUpdateProjectBody`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "deployment_configs": {
    ///        "production": {
    ///          "compatibility_date": "2022-01-01",
    ///          "compatibility_flags": [
    ///            "url_standard"
    ///          ],
    ///          "env_vars": {
    ///            "NODE_VERSION": {
    ///              "value": "22"
    ///            },
    ///            "delete_this_env_var": null,
    ///            "secret_var": {
    ///              "type": "secret_text",
    ///              "value": "A_CMS_API_TOKEN"
    ///            }
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "build_config": {
    ///      "description": "Configs for the project build process.",
    ///      "type": "object",
    ///      "properties": {
    ///        "build_caching": {
    ///          "description": "Enable build caching for the project.",
    ///          "examples": [
    ///            true
    ///          ],
    ///          "type": "boolean",
    ///          "x-auditable": true
    ///        },
    ///        "build_command": {
    ///          "description": "Command used to build project.",
    ///          "examples": [
    ///            "npm run build"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "destination_dir": {
    ///          "description": "Output directory of the build.",
    ///          "examples": [
    ///            "build"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "root_dir": {
    ///          "description": "Directory to run the command.",
    ///          "examples": [
    ///            "/"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "web_analytics_tag": {
    ///          "description": "The classifying tag for analytics.",
    ///          "examples": [
    ///            "cee1c73f6e4743d0b5e6bb1a0bcaabcc"
    ///          ],
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ],
    ///          "x-auditable": true
    ///        },
    ///        "web_analytics_token": {
    ///          "description": "The auth token for analytics.",
    ///          "examples": [
    ///            "021e1057c18547eca7b79f2516f06o7x"
    ///          ],
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ],
    ///          "x-sensitive": true
    ///        }
    ///      }
    ///    },
    ///    "deployment_configs": {
    ///      "description": "Configs for deployments in a project.",
    ///      "type": "object",
    ///      "properties": {
    ///        "preview": {
    ///          "description": "Configs for preview deploys.",
    ///          "allOf": [
    ///            {
    ///              "$ref":
    /// "#/components/schemas/pages_deployment_config_values_request"
    ///            }
    ///          ]
    ///        },
    ///        "production": {
    ///          "description": "Configs for production deploys.",
    ///          "allOf": [
    ///            {
    ///              "$ref":
    /// "#/components/schemas/pages_deployment_config_values_request"
    ///            }
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    "name": {
    ///      "description": "Name of the project.",
    ///      "examples": [
    ///        "my-pages-app"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "production_branch": {
    ///      "description": "Production branch of the project. Used to identify
    /// production deployments.",
    ///      "examples": [
    ///        "main"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "source": {
    ///      "description": "Configs for the project source control.",
    ///      "type": "object",
    ///      "required": [
    ///        "config",
    ///        "type"
    ///      ],
    ///      "properties": {
    ///        "config": {
    ///          "type": "object",
    ///          "properties": {
    ///            "deployments_enabled": {
    ///              "description": "Whether to enable automatic deployments
    /// when pushing to the source repository.\nWhen disabled, no deployments
    /// (production or preview) will be triggered automatically.\n",
    ///              "deprecated": true,
    ///              "type": "boolean",
    ///              "x-auditable": true,
    ///              "x-stainless-deprecation-message": "Use
    /// `production_deployments_enabled` and `preview_deployment_setting` for
    /// more granular control."
    ///            },
    ///            "owner": {
    ///              "description": "The owner of the repository.",
    ///              "examples": [
    ///                "my-org"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            },
    ///            "owner_id": {
    ///              "description": "The owner ID of the repository.",
    ///              "examples": [
    ///                "12345678"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            },
    ///            "path_excludes": {
    ///              "description": "A list of paths that should be excluded
    /// from triggering a preview deployment. Wildcard syntax (`*`) is
    /// supported.",
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string",
    ///                "x-auditable": true
    ///              }
    ///            },
    ///            "path_includes": {
    ///              "description": "A list of paths that should be watched to
    /// trigger a preview deployment. Wildcard syntax (`*`) is supported.",
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string",
    ///                "x-auditable": true
    ///              }
    ///            },
    ///            "pr_comments_enabled": {
    ///              "description": "Whether to enable PR comments.",
    ///              "type": "boolean",
    ///              "x-auditable": true
    ///            },
    ///            "preview_branch_excludes": {
    ///              "description": "A list of branches that should not trigger
    /// a preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string",
    ///                "x-auditable": true
    ///              }
    ///            },
    ///            "preview_branch_includes": {
    ///              "description": "A list of branches that should trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string",
    ///                "x-auditable": true
    ///              }
    ///            },
    ///            "preview_deployment_setting": {
    ///              "description": "Controls whether commits to preview
    /// branches trigger a preview deployment.",
    ///              "type": "string",
    ///              "enum": [
    ///                "all",
    ///                "none",
    ///                "custom"
    ///              ],
    ///              "x-auditable": true
    ///            },
    ///            "production_branch": {
    ///              "description": "The production branch of the repository.",
    ///              "examples": [
    ///                "main"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            },
    ///            "production_deployments_enabled": {
    ///              "description": "Whether to trigger a production deployment
    /// on commits to the production branch.",
    ///              "type": "boolean",
    ///              "x-auditable": true
    ///            },
    ///            "repo_id": {
    ///              "description": "The ID of the repository.",
    ///              "examples": [
    ///                "12345678"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            },
    ///            "repo_name": {
    ///              "description": "The name of the repository.",
    ///              "examples": [
    ///                "my-repo"
    ///              ],
    ///              "type": "string",
    ///              "x-auditable": true
    ///            }
    ///          }
    ///        },
    ///        "type": {
    ///          "description": "The source control management provider.",
    ///          "examples": [
    ///            "github"
    ///          ],
    ///          "type": "string",
    ///          "enum": [
    ///            "github",
    ///            "gitlab"
    ///          ],
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectUpdateProjectBody {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub build_config: ::std::option::Option<PagesProjectUpdateProjectBodyBuildConfig>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub deployment_configs:
            ::std::option::Option<PagesProjectUpdateProjectBodyDeploymentConfigs>,
        ///Name of the project.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        ///Production branch of the project. Used to identify production
        /// deployments.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub production_branch: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<PagesProjectUpdateProjectBodySource>,
    }

    impl ::std::default::Default for PagesProjectUpdateProjectBody {
        fn default() -> Self {
            Self {
                build_config: Default::default(),
                deployment_configs: Default::default(),
                name: Default::default(),
                production_branch: Default::default(),
                source: Default::default(),
            }
        }
    }

    ///Configs for the project build process.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Configs for the project build process.",
    ///  "type": "object",
    ///  "properties": {
    ///    "build_caching": {
    ///      "description": "Enable build caching for the project.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "build_command": {
    ///      "description": "Command used to build project.",
    ///      "examples": [
    ///        "npm run build"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "destination_dir": {
    ///      "description": "Output directory of the build.",
    ///      "examples": [
    ///        "build"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "root_dir": {
    ///      "description": "Directory to run the command.",
    ///      "examples": [
    ///        "/"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "web_analytics_tag": {
    ///      "description": "The classifying tag for analytics.",
    ///      "examples": [
    ///        "cee1c73f6e4743d0b5e6bb1a0bcaabcc"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "web_analytics_token": {
    ///      "description": "The auth token for analytics.",
    ///      "examples": [
    ///        "021e1057c18547eca7b79f2516f06o7x"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "x-sensitive": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectUpdateProjectBodyBuildConfig {
        ///Enable build caching for the project.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub build_caching: ::std::option::Option<bool>,
        ///Command used to build project.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub build_command: ::std::option::Option<::std::string::String>,
        ///Output directory of the build.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub destination_dir: ::std::option::Option<::std::string::String>,
        ///Directory to run the command.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub root_dir: ::std::option::Option<::std::string::String>,
        ///The classifying tag for analytics.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub web_analytics_tag: ::std::option::Option<::std::string::String>,
        ///The auth token for analytics.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub web_analytics_token: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for PagesProjectUpdateProjectBodyBuildConfig {
        fn default() -> Self {
            Self {
                build_caching: Default::default(),
                build_command: Default::default(),
                destination_dir: Default::default(),
                root_dir: Default::default(),
                web_analytics_tag: Default::default(),
                web_analytics_token: Default::default(),
            }
        }
    }

    ///Configs for deployments in a project.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Configs for deployments in a project.",
    ///  "type": "object",
    ///  "properties": {
    ///    "preview": {
    ///      "description": "Configs for preview deploys.",
    ///      "allOf": [
    ///        {
    ///          "$ref":
    /// "#/components/schemas/pages_deployment_config_values_request"
    ///        }
    ///      ]
    ///    },
    ///    "production": {
    ///      "description": "Configs for production deploys.",
    ///      "allOf": [
    ///        {
    ///          "$ref":
    /// "#/components/schemas/pages_deployment_config_values_request"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectUpdateProjectBodyDeploymentConfigs {
        ///Configs for preview deploys.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub preview: ::std::option::Option<PagesDeploymentConfigValuesRequest>,
        ///Configs for production deploys.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub production: ::std::option::Option<PagesDeploymentConfigValuesRequest>,
    }

    impl ::std::default::Default for PagesProjectUpdateProjectBodyDeploymentConfigs {
        fn default() -> Self {
            Self {
                preview: Default::default(),
                production: Default::default(),
            }
        }
    }

    ///Configs for the project source control.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Configs for the project source control.",
    ///  "type": "object",
    ///  "required": [
    ///    "config",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "config": {
    ///      "type": "object",
    ///      "properties": {
    ///        "deployments_enabled": {
    ///          "description": "Whether to enable automatic deployments when
    /// pushing to the source repository.\nWhen disabled, no deployments
    /// (production or preview) will be triggered automatically.\n",
    ///          "deprecated": true,
    ///          "type": "boolean",
    ///          "x-auditable": true,
    ///          "x-stainless-deprecation-message": "Use
    /// `production_deployments_enabled` and `preview_deployment_setting` for
    /// more granular control."
    ///        },
    ///        "owner": {
    ///          "description": "The owner of the repository.",
    ///          "examples": [
    ///            "my-org"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "owner_id": {
    ///          "description": "The owner ID of the repository.",
    ///          "examples": [
    ///            "12345678"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "path_excludes": {
    ///          "description": "A list of paths that should be excluded from triggering a preview deployment. Wildcard syntax (`*`) is supported.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "path_includes": {
    ///          "description": "A list of paths that should be watched to
    /// trigger a preview deployment. Wildcard syntax (`*`) is supported.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "pr_comments_enabled": {
    ///          "description": "Whether to enable PR comments.",
    ///          "type": "boolean",
    ///          "x-auditable": true
    ///        },
    ///        "preview_branch_excludes": {
    ///          "description": "A list of branches that should not trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "preview_branch_includes": {
    ///          "description": "A list of branches that should trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "preview_deployment_setting": {
    ///          "description": "Controls whether commits to preview branches
    /// trigger a preview deployment.",
    ///          "type": "string",
    ///          "enum": [
    ///            "all",
    ///            "none",
    ///            "custom"
    ///          ],
    ///          "x-auditable": true
    ///        },
    ///        "production_branch": {
    ///          "description": "The production branch of the repository.",
    ///          "examples": [
    ///            "main"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "production_deployments_enabled": {
    ///          "description": "Whether to trigger a production deployment on
    /// commits to the production branch.",
    ///          "type": "boolean",
    ///          "x-auditable": true
    ///        },
    ///        "repo_id": {
    ///          "description": "The ID of the repository.",
    ///          "examples": [
    ///            "12345678"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "repo_name": {
    ///          "description": "The name of the repository.",
    ///          "examples": [
    ///            "my-repo"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    },
    ///    "type": {
    ///      "description": "The source control management provider.",
    ///      "examples": [
    ///        "github"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "github",
    ///        "gitlab"
    ///      ],
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectUpdateProjectBodySource {
        pub config: PagesProjectUpdateProjectBodySourceConfig,
        ///The source control management provider.
        #[serde(rename = "type")]
        pub type_: PagesProjectUpdateProjectBodySourceType,
    }

    ///`PagesProjectUpdateProjectBodySourceConfig`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "deployments_enabled": {
    ///      "description": "Whether to enable automatic deployments when
    /// pushing to the source repository.\nWhen disabled, no deployments
    /// (production or preview) will be triggered automatically.\n",
    ///      "deprecated": true,
    ///      "type": "boolean",
    ///      "x-auditable": true,
    ///      "x-stainless-deprecation-message": "Use
    /// `production_deployments_enabled` and `preview_deployment_setting` for
    /// more granular control."
    ///    },
    ///    "owner": {
    ///      "description": "The owner of the repository.",
    ///      "examples": [
    ///        "my-org"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "owner_id": {
    ///      "description": "The owner ID of the repository.",
    ///      "examples": [
    ///        "12345678"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "path_excludes": {
    ///      "description": "A list of paths that should be excluded from triggering a preview deployment. Wildcard syntax (`*`) is supported.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "path_includes": {
    ///      "description": "A list of paths that should be watched to trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "pr_comments_enabled": {
    ///      "description": "Whether to enable PR comments.",
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "preview_branch_excludes": {
    ///      "description": "A list of branches that should not trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "preview_branch_includes": {
    ///      "description": "A list of branches that should trigger a preview
    /// deployment. Wildcard syntax (`*`) is supported. Must be used with
    /// `preview_deployment_setting` set to `custom`.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "preview_deployment_setting": {
    ///      "description": "Controls whether commits to preview branches
    /// trigger a preview deployment.",
    ///      "type": "string",
    ///      "enum": [
    ///        "all",
    ///        "none",
    ///        "custom"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "production_branch": {
    ///      "description": "The production branch of the repository.",
    ///      "examples": [
    ///        "main"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "production_deployments_enabled": {
    ///      "description": "Whether to trigger a production deployment on
    /// commits to the production branch.",
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "repo_id": {
    ///      "description": "The ID of the repository.",
    ///      "examples": [
    ///        "12345678"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "repo_name": {
    ///      "description": "The name of the repository.",
    ///      "examples": [
    ///        "my-repo"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectUpdateProjectBodySourceConfig {
        ///Whether to enable automatic deployments when pushing to the source
        /// repository. When disabled, no deployments (production or
        /// preview) will be triggered automatically.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub deployments_enabled: ::std::option::Option<bool>,
        ///The owner of the repository.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub owner: ::std::option::Option<::std::string::String>,
        ///The owner ID of the repository.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub owner_id: ::std::option::Option<::std::string::String>,
        ///A list of paths that should be excluded from triggering a preview
        /// deployment. Wildcard syntax (`*`) is supported.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub path_excludes: ::std::vec::Vec<::std::string::String>,
        ///A list of paths that should be watched to trigger a preview
        /// deployment. Wildcard syntax (`*`) is supported.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub path_includes: ::std::vec::Vec<::std::string::String>,
        ///Whether to enable PR comments.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pr_comments_enabled: ::std::option::Option<bool>,
        ///A list of branches that should not trigger a preview deployment.
        /// Wildcard syntax (`*`) is supported. Must be used with
        /// `preview_deployment_setting` set to `custom`.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub preview_branch_excludes: ::std::vec::Vec<::std::string::String>,
        ///A list of branches that should trigger a preview deployment.
        /// Wildcard syntax (`*`) is supported. Must be used with
        /// `preview_deployment_setting` set to `custom`.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub preview_branch_includes: ::std::vec::Vec<::std::string::String>,
        ///Controls whether commits to preview branches trigger a preview
        /// deployment.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub preview_deployment_setting: ::std::option::Option<
            PagesProjectUpdateProjectBodySourceConfigPreviewDeploymentSetting,
        >,
        ///The production branch of the repository.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub production_branch: ::std::option::Option<::std::string::String>,
        ///Whether to trigger a production deployment on commits to the
        /// production branch.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub production_deployments_enabled: ::std::option::Option<bool>,
        ///The ID of the repository.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub repo_id: ::std::option::Option<::std::string::String>,
        ///The name of the repository.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub repo_name: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for PagesProjectUpdateProjectBodySourceConfig {
        fn default() -> Self {
            Self {
                deployments_enabled: Default::default(),
                owner: Default::default(),
                owner_id: Default::default(),
                path_excludes: Default::default(),
                path_includes: Default::default(),
                pr_comments_enabled: Default::default(),
                preview_branch_excludes: Default::default(),
                preview_branch_includes: Default::default(),
                preview_deployment_setting: Default::default(),
                production_branch: Default::default(),
                production_deployments_enabled: Default::default(),
                repo_id: Default::default(),
                repo_name: Default::default(),
            }
        }
    }

    ///Controls whether commits to preview branches trigger a preview
    /// deployment.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Controls whether commits to preview branches trigger a
    /// preview deployment.",
    ///  "type": "string",
    ///  "enum": [
    ///    "all",
    ///    "none",
    ///    "custom"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesProjectUpdateProjectBodySourceConfigPreviewDeploymentSetting {
        #[serde(rename = "all")]
        All,
        #[serde(rename = "none")]
        None,
        #[serde(rename = "custom")]
        Custom,
    }

    impl ::std::fmt::Display for PagesProjectUpdateProjectBodySourceConfigPreviewDeploymentSetting {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::All => f.write_str("all"),
                Self::None => f.write_str("none"),
                Self::Custom => f.write_str("custom"),
            }
        }
    }

    impl ::std::str::FromStr for PagesProjectUpdateProjectBodySourceConfigPreviewDeploymentSetting {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "all" => Ok(Self::All),
                "none" => Ok(Self::None),
                "custom" => Ok(Self::Custom),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str>
        for PagesProjectUpdateProjectBodySourceConfigPreviewDeploymentSetting
    {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for PagesProjectUpdateProjectBodySourceConfigPreviewDeploymentSetting
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for PagesProjectUpdateProjectBodySourceConfigPreviewDeploymentSetting
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///The source control management provider.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The source control management provider.",
    ///  "examples": [
    ///    "github"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "github",
    ///    "gitlab"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesProjectUpdateProjectBodySourceType {
        #[serde(rename = "github")]
        Github,
        #[serde(rename = "gitlab")]
        Gitlab,
    }

    impl ::std::fmt::Display for PagesProjectUpdateProjectBodySourceType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Github => f.write_str("github"),
                Self::Gitlab => f.write_str("gitlab"),
            }
        }
    }

    impl ::std::str::FromStr for PagesProjectUpdateProjectBodySourceType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "github" => Ok(Self::Github),
                "gitlab" => Ok(Self::Gitlab),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesProjectUpdateProjectBodySourceType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesProjectUpdateProjectBodySourceType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesProjectUpdateProjectBodySourceType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`PagesProjectUpdateProjectResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/pages_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/pages_project"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesProjectUpdateProjectResponse {
        pub errors: PagesMessages,
        pub messages: PagesMessages,
        pub result: PagesProject,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///An encrypted environment variable.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "An encrypted environment variable.",
    ///  "examples": [
    ///    {
    ///      "type": "secret_text",
    ///      "value": ""
    ///    }
    ///  ],
    ///  "type": [
    ///    "object",
    ///    "null"
    ///  ],
    ///  "required": [
    ///    "type",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "secret_text"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "value": {
    ///      "description": "Secret value.",
    ///      "type": "string",
    ///      "x-sensitive": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct PagesSecretTextEnvVar(pub ::std::option::Option<PagesSecretTextEnvVarInner>);
    impl ::std::ops::Deref for PagesSecretTextEnvVar {
        type Target = ::std::option::Option<PagesSecretTextEnvVarInner>;
        fn deref(&self) -> &::std::option::Option<PagesSecretTextEnvVarInner> {
            &self.0
        }
    }

    impl ::std::convert::From<PagesSecretTextEnvVar>
        for ::std::option::Option<PagesSecretTextEnvVarInner>
    {
        fn from(value: PagesSecretTextEnvVar) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::option::Option<PagesSecretTextEnvVarInner>>
        for PagesSecretTextEnvVar
    {
        fn from(value: ::std::option::Option<PagesSecretTextEnvVarInner>) -> Self {
            Self(value)
        }
    }

    ///An encrypted environment variable.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "An encrypted environment variable.",
    ///  "examples": [
    ///    {
    ///      "type": "secret_text",
    ///      "value": ""
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "required": [
    ///    "type",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "secret_text"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "value": {
    ///      "description": "Secret value.",
    ///      "type": "string",
    ///      "x-sensitive": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesSecretTextEnvVarInner {
        #[serde(rename = "type")]
        pub type_: PagesSecretTextEnvVarInnerType,
        ///Secret value.
        pub value: ::std::string::String,
    }

    ///`PagesSecretTextEnvVarInnerType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "secret_text"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesSecretTextEnvVarInnerType {
        #[serde(rename = "secret_text")]
        SecretText,
    }

    impl ::std::fmt::Display for PagesSecretTextEnvVarInnerType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::SecretText => f.write_str("secret_text"),
            }
        }
    }

    impl ::std::str::FromStr for PagesSecretTextEnvVarInnerType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "secret_text" => Ok(Self::SecretText),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesSecretTextEnvVarInnerType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesSecretTextEnvVarInnerType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesSecretTextEnvVarInnerType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Configs for the project source control.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Configs for the project source control.",
    ///  "type": "object",
    ///  "required": [
    ///    "config",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "config": {
    ///      "type": "object",
    ///      "required": [
    ///        "deployments_enabled",
    ///        "owner",
    ///        "owner_id",
    ///        "path_excludes",
    ///        "path_includes",
    ///        "pr_comments_enabled",
    ///        "preview_branch_excludes",
    ///        "preview_branch_includes",
    ///        "preview_deployment_setting",
    ///        "production_branch",
    ///        "production_deployments_enabled",
    ///        "repo_id",
    ///        "repo_name"
    ///      ],
    ///      "properties": {
    ///        "deployments_enabled": {
    ///          "description": "Whether to enable automatic deployments when
    /// pushing to the source repository.\nWhen disabled, no deployments
    /// (production or preview) will be triggered automatically.\n",
    ///          "deprecated": true,
    ///          "type": "boolean",
    ///          "x-auditable": true,
    ///          "x-stainless-deprecation-message": "Use
    /// `production_deployments_enabled` and `preview_deployment_setting` for
    /// more granular control."
    ///        },
    ///        "owner": {
    ///          "description": "The owner of the repository.",
    ///          "examples": [
    ///            "my-org"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "owner_id": {
    ///          "description": "The owner ID of the repository.",
    ///          "examples": [
    ///            "12345678"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "path_excludes": {
    ///          "description": "A list of paths that should be excluded from triggering a preview deployment. Wildcard syntax (`*`) is supported.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "path_includes": {
    ///          "description": "A list of paths that should be watched to
    /// trigger a preview deployment. Wildcard syntax (`*`) is supported.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "pr_comments_enabled": {
    ///          "description": "Whether to enable PR comments.",
    ///          "type": "boolean",
    ///          "x-auditable": true
    ///        },
    ///        "preview_branch_excludes": {
    ///          "description": "A list of branches that should not trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "preview_branch_includes": {
    ///          "description": "A list of branches that should trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "x-auditable": true
    ///          }
    ///        },
    ///        "preview_deployment_setting": {
    ///          "description": "Controls whether commits to preview branches
    /// trigger a preview deployment.",
    ///          "type": "string",
    ///          "enum": [
    ///            "all",
    ///            "none",
    ///            "custom"
    ///          ],
    ///          "x-auditable": true
    ///        },
    ///        "production_branch": {
    ///          "description": "The production branch of the repository.",
    ///          "examples": [
    ///            "main"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "production_deployments_enabled": {
    ///          "description": "Whether to trigger a production deployment on
    /// commits to the production branch.",
    ///          "type": "boolean",
    ///          "x-auditable": true
    ///        },
    ///        "repo_id": {
    ///          "description": "The ID of the repository.",
    ///          "examples": [
    ///            "12345678"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        },
    ///        "repo_name": {
    ///          "description": "The name of the repository.",
    ///          "examples": [
    ///            "my-repo"
    ///          ],
    ///          "type": "string",
    ///          "x-auditable": true
    ///        }
    ///      }
    ///    },
    ///    "type": {
    ///      "description": "The source control management provider.",
    ///      "examples": [
    ///        "github"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "github",
    ///        "gitlab"
    ///      ],
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesSource {
        pub config: PagesSourceConfig,
        ///The source control management provider.
        #[serde(rename = "type")]
        pub type_: PagesSourceType,
    }

    ///`PagesSourceConfig`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "deployments_enabled",
    ///    "owner",
    ///    "owner_id",
    ///    "path_excludes",
    ///    "path_includes",
    ///    "pr_comments_enabled",
    ///    "preview_branch_excludes",
    ///    "preview_branch_includes",
    ///    "preview_deployment_setting",
    ///    "production_branch",
    ///    "production_deployments_enabled",
    ///    "repo_id",
    ///    "repo_name"
    ///  ],
    ///  "properties": {
    ///    "deployments_enabled": {
    ///      "description": "Whether to enable automatic deployments when
    /// pushing to the source repository.\nWhen disabled, no deployments
    /// (production or preview) will be triggered automatically.\n",
    ///      "deprecated": true,
    ///      "type": "boolean",
    ///      "x-auditable": true,
    ///      "x-stainless-deprecation-message": "Use
    /// `production_deployments_enabled` and `preview_deployment_setting` for
    /// more granular control."
    ///    },
    ///    "owner": {
    ///      "description": "The owner of the repository.",
    ///      "examples": [
    ///        "my-org"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "owner_id": {
    ///      "description": "The owner ID of the repository.",
    ///      "examples": [
    ///        "12345678"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "path_excludes": {
    ///      "description": "A list of paths that should be excluded from triggering a preview deployment. Wildcard syntax (`*`) is supported.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "path_includes": {
    ///      "description": "A list of paths that should be watched to trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "pr_comments_enabled": {
    ///      "description": "Whether to enable PR comments.",
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "preview_branch_excludes": {
    ///      "description": "A list of branches that should not trigger a
    /// preview deployment. Wildcard syntax (`*`) is supported. Must be used
    /// with `preview_deployment_setting` set to `custom`.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "preview_branch_includes": {
    ///      "description": "A list of branches that should trigger a preview
    /// deployment. Wildcard syntax (`*`) is supported. Must be used with
    /// `preview_deployment_setting` set to `custom`.",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "x-auditable": true
    ///      }
    ///    },
    ///    "preview_deployment_setting": {
    ///      "description": "Controls whether commits to preview branches
    /// trigger a preview deployment.",
    ///      "type": "string",
    ///      "enum": [
    ///        "all",
    ///        "none",
    ///        "custom"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "production_branch": {
    ///      "description": "The production branch of the repository.",
    ///      "examples": [
    ///        "main"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "production_deployments_enabled": {
    ///      "description": "Whether to trigger a production deployment on
    /// commits to the production branch.",
    ///      "type": "boolean",
    ///      "x-auditable": true
    ///    },
    ///    "repo_id": {
    ///      "description": "The ID of the repository.",
    ///      "examples": [
    ///        "12345678"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    },
    ///    "repo_name": {
    ///      "description": "The name of the repository.",
    ///      "examples": [
    ///        "my-repo"
    ///      ],
    ///      "type": "string",
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesSourceConfig {
        ///Whether to enable automatic deployments when pushing to the source
        /// repository. When disabled, no deployments (production or
        /// preview) will be triggered automatically.
        pub deployments_enabled: bool,
        ///The owner of the repository.
        pub owner: ::std::string::String,
        ///The owner ID of the repository.
        pub owner_id: ::std::string::String,
        ///A list of paths that should be excluded from triggering a preview
        /// deployment. Wildcard syntax (`*`) is supported.
        pub path_excludes: ::std::vec::Vec<::std::string::String>,
        ///A list of paths that should be watched to trigger a preview
        /// deployment. Wildcard syntax (`*`) is supported.
        pub path_includes: ::std::vec::Vec<::std::string::String>,
        ///Whether to enable PR comments.
        pub pr_comments_enabled: bool,
        ///A list of branches that should not trigger a preview deployment.
        /// Wildcard syntax (`*`) is supported. Must be used with
        /// `preview_deployment_setting` set to `custom`.
        pub preview_branch_excludes: ::std::vec::Vec<::std::string::String>,
        ///A list of branches that should trigger a preview deployment.
        /// Wildcard syntax (`*`) is supported. Must be used with
        /// `preview_deployment_setting` set to `custom`.
        pub preview_branch_includes: ::std::vec::Vec<::std::string::String>,
        ///Controls whether commits to preview branches trigger a preview
        /// deployment.
        pub preview_deployment_setting: PagesSourceConfigPreviewDeploymentSetting,
        ///The production branch of the repository.
        pub production_branch: ::std::string::String,
        ///Whether to trigger a production deployment on commits to the
        /// production branch.
        pub production_deployments_enabled: bool,
        ///The ID of the repository.
        pub repo_id: ::std::string::String,
        ///The name of the repository.
        pub repo_name: ::std::string::String,
    }

    ///Controls whether commits to preview branches trigger a preview
    /// deployment.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Controls whether commits to preview branches trigger a
    /// preview deployment.",
    ///  "type": "string",
    ///  "enum": [
    ///    "all",
    ///    "none",
    ///    "custom"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesSourceConfigPreviewDeploymentSetting {
        #[serde(rename = "all")]
        All,
        #[serde(rename = "none")]
        None,
        #[serde(rename = "custom")]
        Custom,
    }

    impl ::std::fmt::Display for PagesSourceConfigPreviewDeploymentSetting {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::All => f.write_str("all"),
                Self::None => f.write_str("none"),
                Self::Custom => f.write_str("custom"),
            }
        }
    }

    impl ::std::str::FromStr for PagesSourceConfigPreviewDeploymentSetting {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "all" => Ok(Self::All),
                "none" => Ok(Self::None),
                "custom" => Ok(Self::Custom),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesSourceConfigPreviewDeploymentSetting {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesSourceConfigPreviewDeploymentSetting {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesSourceConfigPreviewDeploymentSetting {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///The source control management provider.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The source control management provider.",
    ///  "examples": [
    ///    "github"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "github",
    ///    "gitlab"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesSourceType {
        #[serde(rename = "github")]
        Github,
        #[serde(rename = "gitlab")]
        Gitlab,
    }

    impl ::std::fmt::Display for PagesSourceType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Github => f.write_str("github"),
                Self::Gitlab => f.write_str("gitlab"),
            }
        }
    }

    impl ::std::str::FromStr for PagesSourceType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "github" => Ok(Self::Github),
                "gitlab" => Ok(Self::Gitlab),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesSourceType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesSourceType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesSourceType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///The status of the deployment.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The status of the deployment.",
    ///  "readOnly": true,
    ///  "type": "object",
    ///  "required": [
    ///    "ended_on",
    ///    "name",
    ///    "started_on",
    ///    "status"
    ///  ],
    ///  "properties": {
    ///    "ended_on": {
    ///      "description": "When the stage ended.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "2021-03-09T00:58:59.045655"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "format": "date-time",
    ///      "x-auditable": true
    ///    },
    ///    "name": {
    ///      "description": "The current build stage.",
    ///      "examples": [
    ///        "deploy"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "queued",
    ///        "initialize",
    ///        "clone_repo",
    ///        "build",
    ///        "deploy"
    ///      ],
    ///      "x-auditable": true
    ///    },
    ///    "started_on": {
    ///      "description": "When the stage started.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "2021-03-09T00:55:03.923456Z"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "format": "date-time",
    ///      "x-auditable": true
    ///    },
    ///    "status": {
    ///      "description": "State of the current stage.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "success"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "success",
    ///        "idle",
    ///        "active",
    ///        "failure",
    ///        "canceled"
    ///      ],
    ///      "x-auditable": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PagesStage {
        ///When the stage ended.
        pub ended_on: ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
        ///The current build stage.
        pub name: PagesStageName,
        ///When the stage started.
        pub started_on: ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
        ///State of the current stage.
        pub status: PagesStageStatus,
    }

    ///The current build stage.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The current build stage.",
    ///  "examples": [
    ///    "deploy"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "queued",
    ///    "initialize",
    ///    "clone_repo",
    ///    "build",
    ///    "deploy"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesStageName {
        #[serde(rename = "queued")]
        Queued,
        #[serde(rename = "initialize")]
        Initialize,
        #[serde(rename = "clone_repo")]
        CloneRepo,
        #[serde(rename = "build")]
        Build,
        #[serde(rename = "deploy")]
        Deploy,
    }

    impl ::std::fmt::Display for PagesStageName {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Queued => f.write_str("queued"),
                Self::Initialize => f.write_str("initialize"),
                Self::CloneRepo => f.write_str("clone_repo"),
                Self::Build => f.write_str("build"),
                Self::Deploy => f.write_str("deploy"),
            }
        }
    }

    impl ::std::str::FromStr for PagesStageName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "queued" => Ok(Self::Queued),
                "initialize" => Ok(Self::Initialize),
                "clone_repo" => Ok(Self::CloneRepo),
                "build" => Ok(Self::Build),
                "deploy" => Ok(Self::Deploy),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesStageName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesStageName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesStageName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///State of the current stage.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "State of the current stage.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "success"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "success",
    ///    "idle",
    ///    "active",
    ///    "failure",
    ///    "canceled"
    ///  ],
    ///  "x-auditable": true
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
    pub enum PagesStageStatus {
        #[serde(rename = "success")]
        Success,
        #[serde(rename = "idle")]
        Idle,
        #[serde(rename = "active")]
        Active,
        #[serde(rename = "failure")]
        Failure,
        #[serde(rename = "canceled")]
        Canceled,
    }

    impl ::std::fmt::Display for PagesStageStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Success => f.write_str("success"),
                Self::Idle => f.write_str("idle"),
                Self::Active => f.write_str("active"),
                Self::Failure => f.write_str("failure"),
                Self::Canceled => f.write_str("canceled"),
            }
        }
    }

    impl ::std::str::FromStr for PagesStageStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "success" => Ok(Self::Success),
                "idle" => Ok(Self::Idle),
                "active" => Ok(Self::Active),
                "failure" => Ok(Self::Failure),
                "canceled" => Ok(Self::Canceled),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PagesStageStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PagesStageStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PagesStageStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`RegistrarApiApiResponseCommon`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "result",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "$ref": "#/components/schemas/registrar-api_messages"
    ///    },
    ///    "messages": {
    ///      "$ref": "#/components/schemas/registrar-api_messages"
    ///    },
    ///    "result": {
    ///      "oneOf": [
    ///        {
    ///          "type": "object"
    ///        },
    ///        {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "object"
    ///          }
    ///        },
    ///        {
    ///          "type": "string"
    ///        }
    ///      ]
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean",
    ///      "enum": [
    ///        true
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiApiResponseCommon {
        pub errors: RegistrarApiMessages,
        pub messages: RegistrarApiMessages,
        pub result: RegistrarApiApiResponseCommonResult,
        ///Whether the API call was successful
        pub success: bool,
    }

    ///`RegistrarApiApiResponseCommonFailure`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "result",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "examples": [
    ///        [
    ///          {
    ///            "code": 7003,
    ///            "message": "No route for the URI"
    ///          }
    ///        ]
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/registrar-api_messages"
    ///        }
    ///      ],
    ///      "minItems": 1
    ///    },
    ///    "messages": {
    ///      "examples": [
    ///        []
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/registrar-api_messages"
    ///        }
    ///      ]
    ///    },
    ///    "result": {
    ///      "type": [
    ///        "object",
    ///        "null"
    ///      ],
    ///      "enum": [
    ///        null
    ///      ]
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean",
    ///      "enum": [
    ///        false
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiApiResponseCommonFailure {
        pub errors: RegistrarApiMessages,
        pub messages: RegistrarApiMessages,
        pub result: (),
        ///Whether the API call was successful
        pub success: bool,
    }

    ///`RegistrarApiApiResponseCommonResult`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "type": "object"
    ///    },
    ///    {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object"
    ///      }
    ///    },
    ///    {
    ///      "type": "string"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum RegistrarApiApiResponseCommonResult {
        Object(::serde_json::Map<::std::string::String, ::serde_json::Value>),
        Array(::std::vec::Vec<::serde_json::Map<::std::string::String, ::serde_json::Value>>),
        String(::std::string::String),
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for RegistrarApiApiResponseCommonResult
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self::Object(value)
        }
    }

    impl
        ::std::convert::From<
            ::std::vec::Vec<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        > for RegistrarApiApiResponseCommonResult
    {
        fn from(
            value: ::std::vec::Vec<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        ) -> Self {
            Self::Array(value)
        }
    }

    ///Cursor-based pagination metadata. Used by list endpoints that support
    ///cursor pagination. Pass the `cursor` value as a query parameter in the
    ///next request to fetch the next page. An empty string indicates there
    ///are no more pages.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Cursor-based pagination metadata. Used by list
    /// endpoints that support\ncursor pagination. Pass the `cursor` value as a
    /// query parameter in the\nnext request to fetch the next page. An empty
    /// string indicates there\nare no more pages.\n",
    ///  "type": "object",
    ///  "required": [
    ///    "count",
    ///    "cursor",
    ///    "per_page"
    ///  ],
    ///  "properties": {
    ///    "count": {
    ///      "description": "Number of items in the current result set.",
    ///      "examples": [
    ///        20
    ///      ],
    ///      "type": "integer"
    ///    },
    ///    "cursor": {
    ///      "description": "Opaque cursor for fetching the next page. Pass this
    /// value as the\n`cursor` query parameter in a subsequent request. An empty
    /// string\nindicates there are no more pages.\n",
    ///      "examples": [
    ///        "eyJ0IjoiMjAyNS0wNi0xNVQxMjowMDowMC4wMDAwMDBaIiwibiI6ImJyYXZvLm5ldCJ9"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "per_page": {
    ///      "description": "Maximum number of items per page.",
    ///      "examples": [
    ///        20
    ///      ],
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiCursorResultInfo {
        ///Number of items in the current result set.
        pub count: i64,
        ///Opaque cursor for fetching the next page. Pass this value as the
        ///`cursor` query parameter in a subsequent request. An empty string
        ///indicates there are no more pages.
        pub cursor: ::std::string::String,
        ///Maximum number of items per page.
        pub per_page: i64,
    }

    ///Request body for checking domain availability.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Request body for checking domain availability.",
    ///  "type": "object",
    ///  "required": [
    ///    "domains"
    ///  ],
    ///  "properties": {
    ///    "domains": {
    ///      "description": "List of fully qualified domain names (FQDNs) to
    /// check for availability. Each domain must include the extension.\n-
    /// Minimum: 1 domain\n- Maximum: 20 domains per request\n- Domains on
    /// unsupported extensions are returned with `registrable: false` and a
    /// `reason` field\n- Malformed domain names (e.g., missing extension) may
    /// be omitted from the response\n",
    ///      "examples": [
    ///        [
    ///          "example.com",
    ///          "example.net"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      },
    ///      "maxItems": 20,
    ///      "minItems": 1
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiDomainCheckRequest {
        ///List of fully qualified domain names (FQDNs) to check for
        /// availability. Each domain must include the extension.
        /// - Minimum: 1 domain
        /// - Maximum: 20 domains per request
        /// - Domains on unsupported extensions are returned with `registrable:
        ///   false` and a `reason` field
        /// - Malformed domain names (e.g., missing extension) may be omitted
        ///   from the response
        pub domains: ::std::vec::Vec<::std::string::String>,
    }

    ///`RegistrarApiDomainCheckResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/registrar-api_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "description": "Contains the availability check results.",
    ///          "type": "object",
    ///          "required": [
    ///            "domains"
    ///          ],
    ///          "properties": {
    ///            "domains": {
    ///              "description": "Array of domain availability results. Domains on unsupported\nextensions are included with `registrable: false` and a `reason`\nfield. Malformed domain names may be omitted.\n",
    ///              "type": "array",
    ///              "items": {
    ///                "$ref":
    /// "#/components/schemas/registrar-api_domain_check_result"
    ///              }
    ///            }
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiDomainCheckResponse {
        pub errors: RegistrarApiMessages,
        pub messages: RegistrarApiMessages,
        pub result: RegistrarApiDomainCheckResponseResult,
        ///Whether the API call was successful
        pub success: bool,
    }

    ///`RegistrarApiDomainCheckResponseResult`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "domains"
    ///  ],
    ///  "properties": {
    ///    "domains": {
    ///      "description": "Array of domain availability results. Domains on
    /// unsupported\nextensions are included with `registrable: false` and a
    /// `reason`\nfield. Malformed domain names may be omitted.\n",
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/registrar-api_domain_check_result"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiDomainCheckResponseResult {
        ///Array of domain availability results. Domains on unsupported
        ///extensions are included with `registrable: false` and a `reason`
        ///field. Malformed domain names may be omitted.
        pub domains: ::std::vec::Vec<RegistrarApiDomainCheckResult>,
    }

    ///Represents a single authoritative domain availability result returned by
    /// the Check endpoint. Check results reflect current registry status and
    /// should be used immediately before registration.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Represents a single authoritative domain availability result returned by the Check endpoint. Check results reflect current registry status and should be used immediately before registration.",
    ///  "type": "object",
    ///  "required": [
    ///    "name",
    ///    "registrable"
    ///  ],
    ///  "properties": {
    ///    "name": {
    ///      "description": "The fully qualified domain name (FQDN) in punycode
    /// format for internationalized domain names (IDNs).",
    ///      "examples": [
    ///        "example.com"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "pricing": {
    ///      "$ref": "#/components/schemas/registrar-api_pricing"
    ///    },
    ///    "reason": {
    ///      "description": "Present only when `registrable` is `false`. Explains why the domain cannot be registered via this API.\n- `extension_not_supported_via_api`: Cloudflare Registrar supports this extension in the dashboard but it is not yet available for programmatic registration via this API. The user can register via `https://dash.cloudflare.com/{account_id}/domains/registrations`.\n- `extension_not_supported`: This extension is not supported by Cloudflare Registrar at all.\n- `extension_disallows_registration`: The extension's registry has temporarily or permanently frozen new registrations. No registrar can register domains on this extension at this time.\n- `domain_premium`: The domain is premium priced. Premium registration is not currently supported by this API.\n- `domain_unavailable`: The domain is already registered, reserved, or otherwise not available on a supported extension.",
    ///      "examples": [
    ///        "domain_unavailable"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "extension_not_supported_via_api",
    ///        "extension_not_supported",
    ///        "extension_disallows_registration",
    ///        "domain_premium",
    ///        "domain_unavailable"
    ///      ]
    ///    },
    ///    "registrable": {
    ///      "description": "Indicates whether this domain can be registered
    /// programmatically through this API based on a real-time registry
    /// check.\n- `true`: Domain is available for registration. The `pricing`
    /// object will be included.\n- `false`: Domain is not available. See the
    /// `reason` field for why. `tier` may still be present on some
    /// non-registrable results, such as premium domains.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "tier": {
    ///      "description": "The pricing tier for this domain. Always present
    /// when `registrable` is `true`; defaults to `standard` for most domains.
    /// May be absent when `registrable` is `false`.\n- `standard`: Standard
    /// registry pricing\n- `premium`: Premium domain with higher pricing set by
    /// the registry",
    ///      "examples": [
    ///        "standard"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "standard",
    ///        "premium"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiDomainCheckResult {
        ///The fully qualified domain name (FQDN) in punycode format for
        /// internationalized domain names (IDNs).
        pub name: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pricing: ::std::option::Option<RegistrarApiPricing>,
        ///Present only when `registrable` is `false`. Explains why the domain
        /// cannot be registered via this API.
        /// - `extension_not_supported_via_api`: Cloudflare Registrar supports this extension in the dashboard but it is not yet available for programmatic registration via this API. The user can register via `https://dash.cloudflare.com/{account_id}/domains/registrations`.
        /// - `extension_not_supported`: This extension is not supported by
        ///   Cloudflare Registrar at all.
        /// - `extension_disallows_registration`: The extension's registry has
        ///   temporarily or permanently frozen new registrations. No registrar
        ///   can register domains on this extension at this time.
        /// - `domain_premium`: The domain is premium priced. Premium
        ///   registration is not currently supported by this API.
        /// - `domain_unavailable`: The domain is already registered, reserved,
        ///   or otherwise not available on a supported extension.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub reason: ::std::option::Option<RegistrarApiDomainCheckResultReason>,
        ///Indicates whether this domain can be registered programmatically
        /// through this API based on a real-time registry check.
        /// - `true`: Domain is available for registration. The `pricing` object
        ///   will be included.
        /// - `false`: Domain is not available. See the `reason` field for why.
        ///   `tier` may still be present on some non-registrable results, such
        ///   as premium domains.
        pub registrable: bool,
        ///The pricing tier for this domain. Always present when `registrable`
        /// is `true`; defaults to `standard` for most domains. May be absent
        /// when `registrable` is `false`.
        /// - `standard`: Standard registry pricing
        /// - `premium`: Premium domain with higher pricing set by the registry
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tier: ::std::option::Option<RegistrarApiDomainCheckResultTier>,
    }

    ///Present only when `registrable` is `false`. Explains why the domain
    /// cannot be registered via this API.
    /// - `extension_not_supported_via_api`: Cloudflare Registrar supports this extension in the dashboard but it is not yet available for programmatic registration via this API. The user can register via `https://dash.cloudflare.com/{account_id}/domains/registrations`.
    /// - `extension_not_supported`: This extension is not supported by
    ///   Cloudflare Registrar at all.
    /// - `extension_disallows_registration`: The extension's registry has
    ///   temporarily or permanently frozen new registrations. No registrar can
    ///   register domains on this extension at this time.
    /// - `domain_premium`: The domain is premium priced. Premium registration
    ///   is not currently supported by this API.
    /// - `domain_unavailable`: The domain is already registered, reserved, or
    ///   otherwise not available on a supported extension.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Present only when `registrable` is `false`. Explains why the domain cannot be registered via this API.\n- `extension_not_supported_via_api`: Cloudflare Registrar supports this extension in the dashboard but it is not yet available for programmatic registration via this API. The user can register via `https://dash.cloudflare.com/{account_id}/domains/registrations`.\n- `extension_not_supported`: This extension is not supported by Cloudflare Registrar at all.\n- `extension_disallows_registration`: The extension's registry has temporarily or permanently frozen new registrations. No registrar can register domains on this extension at this time.\n- `domain_premium`: The domain is premium priced. Premium registration is not currently supported by this API.\n- `domain_unavailable`: The domain is already registered, reserved, or otherwise not available on a supported extension.",
    ///  "examples": [
    ///    "domain_unavailable"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "extension_not_supported_via_api",
    ///    "extension_not_supported",
    ///    "extension_disallows_registration",
    ///    "domain_premium",
    ///    "domain_unavailable"
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
    pub enum RegistrarApiDomainCheckResultReason {
        #[serde(rename = "extension_not_supported_via_api")]
        ExtensionNotSupportedViaApi,
        #[serde(rename = "extension_not_supported")]
        ExtensionNotSupported,
        #[serde(rename = "extension_disallows_registration")]
        ExtensionDisallowsRegistration,
        #[serde(rename = "domain_premium")]
        DomainPremium,
        #[serde(rename = "domain_unavailable")]
        DomainUnavailable,
    }

    impl ::std::fmt::Display for RegistrarApiDomainCheckResultReason {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::ExtensionNotSupportedViaApi => f.write_str("extension_not_supported_via_api"),
                Self::ExtensionNotSupported => f.write_str("extension_not_supported"),
                Self::ExtensionDisallowsRegistration => {
                    f.write_str("extension_disallows_registration")
                }
                Self::DomainPremium => f.write_str("domain_premium"),
                Self::DomainUnavailable => f.write_str("domain_unavailable"),
            }
        }
    }

    impl ::std::str::FromStr for RegistrarApiDomainCheckResultReason {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "extension_not_supported_via_api" => Ok(Self::ExtensionNotSupportedViaApi),
                "extension_not_supported" => Ok(Self::ExtensionNotSupported),
                "extension_disallows_registration" => Ok(Self::ExtensionDisallowsRegistration),
                "domain_premium" => Ok(Self::DomainPremium),
                "domain_unavailable" => Ok(Self::DomainUnavailable),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for RegistrarApiDomainCheckResultReason {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for RegistrarApiDomainCheckResultReason {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for RegistrarApiDomainCheckResultReason {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///The pricing tier for this domain. Always present when `registrable` is
    /// `true`; defaults to `standard` for most domains. May be absent when
    /// `registrable` is `false`.
    /// - `standard`: Standard registry pricing
    /// - `premium`: Premium domain with higher pricing set by the registry
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The pricing tier for this domain. Always present when
    /// `registrable` is `true`; defaults to `standard` for most domains. May be
    /// absent when `registrable` is `false`.\n- `standard`: Standard registry
    /// pricing\n- `premium`: Premium domain with higher pricing set by the
    /// registry",
    ///  "examples": [
    ///    "standard"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "standard",
    ///    "premium"
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
    pub enum RegistrarApiDomainCheckResultTier {
        #[serde(rename = "standard")]
        Standard,
        #[serde(rename = "premium")]
        Premium,
    }

    impl ::std::fmt::Display for RegistrarApiDomainCheckResultTier {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Standard => f.write_str("standard"),
                Self::Premium => f.write_str("premium"),
            }
        }
    }

    impl ::std::str::FromStr for RegistrarApiDomainCheckResultTier {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "standard" => Ok(Self::Standard),
                "premium" => Ok(Self::Premium),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for RegistrarApiDomainCheckResultTier {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for RegistrarApiDomainCheckResultTier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for RegistrarApiDomainCheckResultTier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Fully qualified domain name (FQDN) including the extension
    ///(e.g., `example.com`, `mybrand.app`). The domain name uniquely
    ///identifies a registration — the same domain cannot be registered
    ///twice, making it a natural idempotency key for registration requests.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Fully qualified domain name (FQDN) including the
    /// extension\n(e.g., `example.com`, `mybrand.app`). The domain name
    /// uniquely\nidentifies a registration — the same domain cannot be
    /// registered\ntwice, making it a natural idempotency key for registration
    /// requests.\n",
    ///  "examples": [
    ///    "example.com"
    ///  ],
    ///  "type": "string",
    ///  "x-auditable": true
    ///}
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    #[serde(transparent)]
    pub struct RegistrarApiDomainName(pub ::std::string::String);
    impl ::std::ops::Deref for RegistrarApiDomainName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<RegistrarApiDomainName> for ::std::string::String {
        fn from(value: RegistrarApiDomainName) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::string::String> for RegistrarApiDomainName {
        fn from(value: ::std::string::String) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for RegistrarApiDomainName {
        type Err = ::std::convert::Infallible;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::fmt::Display for RegistrarApiDomainName {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Identifier
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Identifier",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "023e105f4ecef8ad9ca31a8372d0c353"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct RegistrarApiIdentifier(::std::string::String);
    impl ::std::ops::Deref for RegistrarApiIdentifier {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<RegistrarApiIdentifier> for ::std::string::String {
        fn from(value: RegistrarApiIdentifier) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for RegistrarApiIdentifier {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for RegistrarApiIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for RegistrarApiIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for RegistrarApiIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for RegistrarApiIdentifier {
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

    ///`RegistrarApiMessages`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    []
    ///  ],
    ///  "type": "array",
    ///  "items": {
    ///    "type": "object",
    ///    "uniqueItems": true,
    ///    "required": [
    ///      "code",
    ///      "message"
    ///    ],
    ///    "properties": {
    ///      "code": {
    ///        "type": "integer",
    ///        "minimum": 1000.0
    ///      },
    ///      "message": {
    ///        "type": "string"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct RegistrarApiMessages(pub ::std::vec::Vec<RegistrarApiMessagesItem>);
    impl ::std::ops::Deref for RegistrarApiMessages {
        type Target = ::std::vec::Vec<RegistrarApiMessagesItem>;
        fn deref(&self) -> &::std::vec::Vec<RegistrarApiMessagesItem> {
            &self.0
        }
    }

    impl ::std::convert::From<RegistrarApiMessages> for ::std::vec::Vec<RegistrarApiMessagesItem> {
        fn from(value: RegistrarApiMessages) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::vec::Vec<RegistrarApiMessagesItem>> for RegistrarApiMessages {
        fn from(value: ::std::vec::Vec<RegistrarApiMessagesItem>) -> Self {
            Self(value)
        }
    }

    ///`RegistrarApiMessagesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiMessagesItem {
        pub code: i64,
        pub message: ::std::string::String,
    }

    ///Annual pricing information for a registrable domain. This object is only
    ///present when `registrable` is `true`. All prices are per year and
    /// returned as strings to preserve decimal precision.
    ///
    ///`registration_cost` and `renewal_cost` are frequently the same value,
    /// but may differ — especially for premium domains where registries set
    /// different rates for initial registration vs. renewal. For a
    /// multi-year registration (e.g., 4 years), the first year is charged
    /// at `registration_cost` and each subsequent year at `renewal_cost`.
    /// Registry pricing may change over time; the values returned here
    /// reflect the current registry rate. Premium pricing may be surfaced
    /// by Search and Check, but premium registration is not currently
    /// supported by this API.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Annual pricing information for a registrable domain.
    /// This object is only\npresent when `registrable` is `true`. All prices
    /// are per year and returned\nas strings to preserve decimal
    /// precision.\n\n`registration_cost` and `renewal_cost` are frequently the
    /// same value, but\nmay differ — especially for premium domains where
    /// registries set different\nrates for initial registration vs. renewal.
    /// For a multi-year registration\n(e.g., 4 years), the first year is
    /// charged at `registration_cost` and each\nsubsequent year at
    /// `renewal_cost`. Registry pricing may change over time;\nthe values
    /// returned here reflect the current registry rate. Premium pricing\nmay be
    /// surfaced by Search and Check, but premium registration is not
    /// currently\nsupported by this API.\n",
    ///  "type": "object",
    ///  "required": [
    ///    "currency",
    ///    "registration_cost",
    ///    "renewal_cost"
    ///  ],
    ///  "properties": {
    ///    "currency": {
    ///      "description": "ISO-4217 currency code for the prices (e.g.,
    /// \"USD\", \"EUR\", \"GBP\").",
    ///      "examples": [
    ///        "USD"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "registration_cost": {
    ///      "description": "The first-year cost to register this domain. For
    /// premium domains\n(`tier: premium`), this price is set by the registry
    /// and may be\nsignificantly higher than standard pricing. For
    /// multi-year\nregistrations, this cost applies to the first year only;
    /// subsequent\nyears are charged at `renewal_cost`.\n",
    ///      "examples": [
    ///        "8.57"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "renewal_cost": {
    ///      "description": "Per-year renewal cost for this domain. Applied to
    /// each year beyond\nthe first year of a multi-year registration, and to
    /// each annual\nauto-renewal thereafter. May differ from
    /// `registration_cost`,\nespecially for premium domains where initial
    /// registration often\ncosts more than renewals.\n",
    ///      "examples": [
    ///        "8.57"
    ///      ],
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiPricing {
        ///ISO-4217 currency code for the prices (e.g., "USD", "EUR", "GBP").
        pub currency: ::std::string::String,
        ///The first-year cost to register this domain. For premium domains
        ///(`tier: premium`), this price is set by the registry and may be
        ///significantly higher than standard pricing. For multi-year
        ///registrations, this cost applies to the first year only; subsequent
        ///years are charged at `renewal_cost`.
        pub registration_cost: ::std::string::String,
        ///Per-year renewal cost for this domain. Applied to each year beyond
        ///the first year of a multi-year registration, and to each annual
        ///auto-renewal thereafter. May differ from `registration_cost`,
        ///especially for premium domains where initial registration often
        ///costs more than renewals.
        pub renewal_cost: ::std::string::String,
    }

    ///A domain registration resource representing the current state of a
    /// registered domain.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A domain registration resource representing the current
    /// state of a registered domain.",
    ///  "type": "object",
    ///  "required": [
    ///    "auto_renew",
    ///    "created_at",
    ///    "domain_name",
    ///    "expires_at",
    ///    "locked",
    ///    "privacy_mode",
    ///    "status"
    ///  ],
    ///  "properties": {
    ///    "auto_renew": {
    ///      "description": "Whether the domain will be automatically renewed
    /// before expiration.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "created_at": {
    ///      "description": "When the domain was registered. Present when the
    /// registration resource exists.",
    ///      "examples": [
    ///        "2025-01-15T10:00:00Z"
    ///      ],
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "domain_name": {
    ///      "$ref": "#/components/schemas/registrar-api_domain_name"
    ///    },
    ///    "expires_at": {
    ///      "description": "When the domain registration expires. Present when
    /// the registration is ready; may be null only while `status` is
    /// `registration_pending`.",
    ///      "examples": [
    ///        "2026-01-15T10:00:00Z"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "format": "date-time"
    ///    },
    ///    "locked": {
    ///      "description": "Whether the domain is locked for transfer.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "privacy_mode": {
    ///      "description": "Current WHOIS privacy mode for the registration.",
    ///      "examples": [
    ///        "redaction"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "false",
    ///        "redaction"
    ///      ]
    ///    },
    ///    "status": {
    ///      "description": "Current registration status.\n- `active`: Domain is
    /// registered and operational\n- `registration_pending`: Registration is in
    /// progress\n- `expired`: Domain has expired\n- `suspended`: Domain is
    /// suspended by the registry\n- `redemption_period`: Domain is in the
    /// redemption grace period\n- `pending_delete`: Domain is pending deletion
    /// by the registry\n",
    ///      "examples": [
    ///        "active"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "active",
    ///        "registration_pending",
    ///        "expired",
    ///        "suspended",
    ///        "redemption_period",
    ///        "pending_delete"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiRegistration {
        ///Whether the domain will be automatically renewed before expiration.
        pub auto_renew: bool,
        ///When the domain was registered. Present when the registration
        /// resource exists.
        pub created_at: ::chrono::DateTime<::chrono::offset::Utc>,
        pub domain_name: RegistrarApiDomainName,
        ///When the domain registration expires. Present when the registration
        /// is ready; may be null only while `status` is `registration_pending`.
        pub expires_at: ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
        ///Whether the domain is locked for transfer.
        pub locked: bool,
        ///Current WHOIS privacy mode for the registration.
        pub privacy_mode: RegistrarApiRegistrationPrivacyMode,
        ///Current registration status.
        /// - `active`: Domain is registered and operational
        /// - `registration_pending`: Registration is in progress
        /// - `expired`: Domain has expired
        /// - `suspended`: Domain is suspended by the registry
        /// - `redemption_period`: Domain is in the redemption grace period
        /// - `pending_delete`: Domain is pending deletion by the registry
        pub status: RegistrarApiRegistrationStatus,
    }

    ///Registrant contact data for the domain registration. This information
    ///is submitted to the domain registry and, depending on extension and
    ///privacy settings, may appear in public WHOIS records.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Registrant contact data for the domain registration. This information\nis submitted to the domain registry and, depending on extension and\nprivacy settings, may appear in public WHOIS records.\n",
    ///  "type": "object",
    ///  "required": [
    ///    "email",
    ///    "phone",
    ///    "postal_info"
    ///  ],
    ///  "properties": {
    ///    "email": {
    ///      "description": "Email address for the registrant. Used for
    /// domain-related\ncommunications from the registry, including ownership
    /// verification\nand renewal notices.\n",
    ///      "examples": [
    ///        "ada@example.com"
    ///      ],
    ///      "type": "string",
    ///      "format": "email"
    ///    },
    ///    "fax": {
    ///      "description": "Fax number in E.164 format (e.g., `+1.5555555555`).
    /// Optional.\nMost registrations do not require a fax number.\n",
    ///      "examples": [
    ///        "+1.5555555555"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "phone": {
    ///      "description": "Phone number in E.164 format:
    /// `+{country_code}.{number}` with no\nspaces or dashes. Examples:
    /// `+1.5555555555` (US), `+44.2071234567`\n(UK), `+81.312345678`
    /// (Japan).\n",
    ///      "examples": [
    ///        "+1.5555555555"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "postal_info": {
    ///      "$ref":
    /// "#/components/schemas/registrar-api_registration_contact_postal_info"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiRegistrationContact {
        ///Email address for the registrant. Used for domain-related
        ///communications from the registry, including ownership verification
        ///and renewal notices.
        pub email: ::std::string::String,
        ///Fax number in E.164 format (e.g., `+1.5555555555`). Optional.
        ///Most registrations do not require a fax number.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub fax: ::std::option::Option<::std::string::String>,
        ///Phone number in E.164 format: `+{country_code}.{number}` with no
        ///spaces or dashes. Examples: `+1.5555555555` (US), `+44.2071234567`
        ///(UK), `+81.312345678` (Japan).
        pub phone: ::std::string::String,
        pub postal_info: RegistrarApiRegistrationContactPostalInfo,
    }

    ///Physical mailing address for the registrant contact.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Physical mailing address for the registrant contact.",
    ///  "type": "object",
    ///  "required": [
    ///    "city",
    ///    "country_code",
    ///    "postal_code",
    ///    "state",
    ///    "street"
    ///  ],
    ///  "properties": {
    ///    "city": {
    ///      "description": "City or locality name.",
    ///      "examples": [
    ///        "Austin"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "country_code": {
    ///      "description": "Two-letter country code per ISO 3166-1 alpha-2
    /// (e.g., `US`, `GB`, `CA`, `DE`).",
    ///      "examples": [
    ///        "US"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "postal_code": {
    ///      "description": "Postal or ZIP code.",
    ///      "examples": [
    ///        "78701"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "state": {
    ///      "description": "State, province, or region. Use the standard
    /// abbreviation where applicable (e.g., `TX` for Texas, `ON` for
    /// Ontario).",
    ///      "examples": [
    ///        "TX"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "street": {
    ///      "description": "Street address including building/suite number.",
    ///      "examples": [
    ///        "123 Main St"
    ///      ],
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiRegistrationContactAddress {
        ///City or locality name.
        pub city: ::std::string::String,
        ///Two-letter country code per ISO 3166-1 alpha-2 (e.g., `US`, `GB`,
        /// `CA`, `DE`).
        pub country_code: ::std::string::String,
        ///Postal or ZIP code.
        pub postal_code: ::std::string::String,
        ///State, province, or region. Use the standard abbreviation where
        /// applicable (e.g., `TX` for Texas, `ON` for Ontario).
        pub state: ::std::string::String,
        ///Street address including building/suite number.
        pub street: ::std::string::String,
    }

    ///Postal/mailing information for the registrant contact.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Postal/mailing information for the registrant
    /// contact.",
    ///  "type": "object",
    ///  "required": [
    ///    "address",
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "address": {
    ///      "$ref":
    /// "#/components/schemas/registrar-api_registration_contact_address"
    ///    },
    ///    "name": {
    ///      "description": "Full legal name of the registrant (individual or
    /// authorized representative).",
    ///      "examples": [
    ///        "Ada Lovelace"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "organization": {
    ///      "description": "Organization or company name. Optional for
    /// individual registrants.",
    ///      "examples": [
    ///        "Example Inc"
    ///      ],
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiRegistrationContactPostalInfo {
        pub address: RegistrarApiRegistrationContactAddress,
        ///Full legal name of the registrant (individual or authorized
        /// representative).
        pub name: ::std::string::String,
        ///Organization or company name. Optional for individual registrants.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub organization: ::std::option::Option<::std::string::String>,
    }

    ///Contact data for the registration request.
    ///
    ///If the `contacts` object is omitted entirely from the request, or if
    ///`contacts.registrant` is not provided, the system will use the account's
    ///default address book entry as the registrant contact. This default must
    /// be pre-configured by the account owner at
    ///`https://dash.cloudflare.com/{account_id}/domains/registrations`, where
    ///they can create or update the address book entry and accept the required
    ///agreement. No API exists for managing address book entries at this time.
    ///
    ///If no default address book entry exists and no registrant contact is
    ///provided, the registration request will fail with a validation error.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contact data for the registration request.\n\nIf the `contacts` object is omitted entirely from the request, or if\n`contacts.registrant` is not provided, the system will use the account's\ndefault address book entry as the registrant contact. This default must be\npre-configured by the account owner at\n`https://dash.cloudflare.com/{account_id}/domains/registrations`, where\nthey can create or update the address book entry and accept the required\nagreement. No API exists for managing address book entries at this time.\n\nIf no default address book entry exists and no registrant contact is\nprovided, the registration request will fail with a validation error.\n",
    ///  "type": "object",
    ///  "properties": {
    ///    "registrant": {
    ///      "$ref": "#/components/schemas/registrar-api_registration_contact"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiRegistrationContacts {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub registrant: ::std::option::Option<RegistrarApiRegistrationContact>,
    }

    impl ::std::default::Default for RegistrarApiRegistrationContacts {
        fn default() -> Self {
            Self {
                registrant: Default::default(),
            }
        }
    }

    ///`RegistrarApiRegistrationCreateRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "domain_name"
    ///  ],
    ///  "properties": {
    ///    "auto_renew": {
    ///      "description": "Enable or disable automatic renewal. Defaults to
    /// `false` if omitted.\nSetting this field to `true` is an explicit opt-in
    /// authorizing\nCloudflare to charge the account's default payment method
    /// up to 30\ndays before domain expiry to renew the domain
    /// automatically.\nRenewal pricing may change over time based on registry
    /// pricing.\n",
    ///      "default": false,
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "contacts": {
    ///      "$ref": "#/components/schemas/registrar-api_registration_contacts"
    ///    },
    ///    "domain_name": {
    ///      "$ref": "#/components/schemas/registrar-api_domain_name"
    ///    },
    ///    "privacy_mode": {
    ///      "description": "WHOIS privacy mode for the registration. Defaults
    /// to `redaction`.\n- `off`: Do not request WHOIS privacy.\n- `redaction`:
    /// Request WHOIS redaction where supported by the extension.\n  Some
    /// extensions do not support privacy/redaction.\n",
    ///      "default": "redaction",
    ///      "examples": [
    ///        "redaction"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "false",
    ///        "redaction"
    ///      ]
    ///    },
    ///    "years": {
    ///      "description": "Number of years to register (1–10). If omitted,
    /// defaults to the\nminimum registration period required by the registry
    /// for this\nextension. For most extensions this is 1 year, but some
    /// extensions\nrequire longer minimum terms (e.g., `.ai` requires a minimum
    /// of\n2 years).\n\nThe registry for each extension may also enforce its
    /// own maximum\nregistration term. If the requested value exceeds the
    /// registry's\nmaximum, the registration will be rejected. When in doubt,
    /// use the\ndefault by omitting this field.\n",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "maximum": 10.0,
    ///      "minimum": 1.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiRegistrationCreateRequest {
        ///Enable or disable automatic renewal. Defaults to `false` if omitted.
        ///Setting this field to `true` is an explicit opt-in authorizing
        ///Cloudflare to charge the account's default payment method up to 30
        ///days before domain expiry to renew the domain automatically.
        ///Renewal pricing may change over time based on registry pricing.
        #[serde(default)]
        pub auto_renew: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub contacts: ::std::option::Option<RegistrarApiRegistrationContacts>,
        pub domain_name: RegistrarApiDomainName,
        ///WHOIS privacy mode for the registration. Defaults to `redaction`.
        /// - `off`: Do not request WHOIS privacy.
        /// - `redaction`: Request WHOIS redaction where supported by the
        ///   extension.
        ///  Some extensions do not support privacy/redaction.
        #[serde(default = "defaults::registrar_api_registration_create_request_privacy_mode")]
        pub privacy_mode: RegistrarApiRegistrationCreateRequestPrivacyMode,
        ///Number of years to register (1–10). If omitted, defaults to the
        ///minimum registration period required by the registry for this
        ///extension. For most extensions this is 1 year, but some extensions
        ///require longer minimum terms (e.g., `.ai` requires a minimum of
        ///2 years).
        ///
        ///The registry for each extension may also enforce its own maximum
        ///registration term. If the requested value exceeds the registry's
        ///maximum, the registration will be rejected. When in doubt, use the
        ///default by omitting this field.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub years: ::std::option::Option<::std::num::NonZeroU64>,
    }

    ///WHOIS privacy mode for the registration. Defaults to `redaction`.
    /// - `off`: Do not request WHOIS privacy.
    /// - `redaction`: Request WHOIS redaction where supported by the extension.
    ///  Some extensions do not support privacy/redaction.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "WHOIS privacy mode for the registration. Defaults to
    /// `redaction`.\n- `off`: Do not request WHOIS privacy.\n- `redaction`:
    /// Request WHOIS redaction where supported by the extension.\n  Some
    /// extensions do not support privacy/redaction.\n",
    ///  "default": "redaction",
    ///  "examples": [
    ///    "redaction"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "false",
    ///    "redaction"
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
    pub enum RegistrarApiRegistrationCreateRequestPrivacyMode {
        #[serde(rename = "false")]
        False,
        #[serde(rename = "redaction")]
        Redaction,
    }

    impl ::std::fmt::Display for RegistrarApiRegistrationCreateRequestPrivacyMode {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::False => f.write_str("false"),
                Self::Redaction => f.write_str("redaction"),
            }
        }
    }

    impl ::std::str::FromStr for RegistrarApiRegistrationCreateRequestPrivacyMode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "false" => Ok(Self::False),
                "redaction" => Ok(Self::Redaction),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for RegistrarApiRegistrationCreateRequestPrivacyMode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for RegistrarApiRegistrationCreateRequestPrivacyMode
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for RegistrarApiRegistrationCreateRequestPrivacyMode
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for RegistrarApiRegistrationCreateRequestPrivacyMode {
        fn default() -> Self {
            RegistrarApiRegistrationCreateRequestPrivacyMode::Redaction
        }
    }

    ///Current WHOIS privacy mode for the registration.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Current WHOIS privacy mode for the registration.",
    ///  "examples": [
    ///    "redaction"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "false",
    ///    "redaction"
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
    pub enum RegistrarApiRegistrationPrivacyMode {
        #[serde(rename = "false")]
        False,
        #[serde(rename = "redaction")]
        Redaction,
    }

    impl ::std::fmt::Display for RegistrarApiRegistrationPrivacyMode {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::False => f.write_str("false"),
                Self::Redaction => f.write_str("redaction"),
            }
        }
    }

    impl ::std::str::FromStr for RegistrarApiRegistrationPrivacyMode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "false" => Ok(Self::False),
                "redaction" => Ok(Self::Redaction),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for RegistrarApiRegistrationPrivacyMode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for RegistrarApiRegistrationPrivacyMode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for RegistrarApiRegistrationPrivacyMode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`RegistrarApiRegistrationResponseCollection`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/registrar-api_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "result",
    ///        "result_info"
    ///      ],
    ///      "properties": {
    ///        "result": {
    ///          "type": "array",
    ///          "items": {
    ///            "$ref": "#/components/schemas/registrar-api_registration"
    ///          }
    ///        },
    ///        "result_info": {
    ///          "$ref": "#/components/schemas/registrar-api_cursor_result_info"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiRegistrationResponseCollection {
        pub errors: RegistrarApiMessages,
        pub messages: RegistrarApiMessages,
        pub result: ::std::vec::Vec<RegistrarApiRegistration>,
        pub result_info: RegistrarApiCursorResultInfo,
        ///Whether the API call was successful
        pub success: bool,
    }

    ///`RegistrarApiRegistrationResponseSingle`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/registrar-api_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/registrar-api_registration"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiRegistrationResponseSingle {
        pub errors: RegistrarApiMessages,
        pub messages: RegistrarApiMessages,
        pub result: RegistrarApiRegistration,
        ///Whether the API call was successful
        pub success: bool,
    }

    ///Current registration status.
    /// - `active`: Domain is registered and operational
    /// - `registration_pending`: Registration is in progress
    /// - `expired`: Domain has expired
    /// - `suspended`: Domain is suspended by the registry
    /// - `redemption_period`: Domain is in the redemption grace period
    /// - `pending_delete`: Domain is pending deletion by the registry
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Current registration status.\n- `active`: Domain is
    /// registered and operational\n- `registration_pending`: Registration is in
    /// progress\n- `expired`: Domain has expired\n- `suspended`: Domain is
    /// suspended by the registry\n- `redemption_period`: Domain is in the
    /// redemption grace period\n- `pending_delete`: Domain is pending deletion
    /// by the registry\n",
    ///  "examples": [
    ///    "active"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "active",
    ///    "registration_pending",
    ///    "expired",
    ///    "suspended",
    ///    "redemption_period",
    ///    "pending_delete"
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
    pub enum RegistrarApiRegistrationStatus {
        #[serde(rename = "active")]
        Active,
        #[serde(rename = "registration_pending")]
        RegistrationPending,
        #[serde(rename = "expired")]
        Expired,
        #[serde(rename = "suspended")]
        Suspended,
        #[serde(rename = "redemption_period")]
        RedemptionPeriod,
        #[serde(rename = "pending_delete")]
        PendingDelete,
    }

    impl ::std::fmt::Display for RegistrarApiRegistrationStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Active => f.write_str("active"),
                Self::RegistrationPending => f.write_str("registration_pending"),
                Self::Expired => f.write_str("expired"),
                Self::Suspended => f.write_str("suspended"),
                Self::RedemptionPeriod => f.write_str("redemption_period"),
                Self::PendingDelete => f.write_str("pending_delete"),
            }
        }
    }

    impl ::std::str::FromStr for RegistrarApiRegistrationStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "active" => Ok(Self::Active),
                "registration_pending" => Ok(Self::RegistrationPending),
                "expired" => Ok(Self::Expired),
                "suspended" => Ok(Self::Suspended),
                "redemption_period" => Ok(Self::RedemptionPeriod),
                "pending_delete" => Ok(Self::PendingDelete),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for RegistrarApiRegistrationStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for RegistrarApiRegistrationStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for RegistrarApiRegistrationStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Request to update an existing domain registration.
    ///
    ///This endpoint currently supports updating `auto_renew` only.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Request to update an existing domain
    /// registration.\n\nThis endpoint currently supports updating `auto_renew`
    /// only.\n",
    ///  "type": "object",
    ///  "minProperties": 1,
    ///  "properties": {
    ///    "auto_renew": {
    ///      "description": "Enable or disable automatic renewal.\nSetting this
    /// field to `true` authorizes Cloudflare to charge the\naccount's default
    /// payment method up to 30 days before domain expiry\nto renew the domain
    /// automatically. Renewal pricing may change over\ntime based on registry
    /// pricing.\n",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiRegistrationUpdateRequest {
        ///Enable or disable automatic renewal.
        ///Setting this field to `true` authorizes Cloudflare to charge the
        ///account's default payment method up to 30 days before domain expiry
        ///to renew the domain automatically. Renewal pricing may change over
        ///time based on registry pricing.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub auto_renew: ::std::option::Option<bool>,
    }

    impl ::std::default::Default for RegistrarApiRegistrationUpdateRequest {
        fn default() -> Self {
            Self {
                auto_renew: Default::default(),
            }
        }
    }

    ///Error details when a workflow reaches the `failed` state. The specific
    ///error codes and messages depend on the workflow type (registration,
    ///update, etc.) and the underlying registry response. These workflow
    ///error codes are separate from immediate HTTP error `errors[].code`
    ///values returned by non-2xx responses. Surface
    ///`error.message` to the user for context.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Error details when a workflow reaches the `failed`
    /// state. The specific\nerror codes and messages depend on the workflow
    /// type (registration,\nupdate, etc.) and the underlying registry response.
    /// These workflow\nerror codes are separate from immediate HTTP error
    /// `errors[].code`\nvalues returned by non-2xx responses.
    /// Surface\n`error.message` to the user for context.\n",
    ///  "type": "object",
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "description": "Machine-readable error code identifying the failure
    /// reason.",
    ///      "examples": [
    ///        "registry_rejected"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "description": "Human-readable explanation of the failure. May
    /// include registry-specific details.",
    ///      "examples": [
    ///        "Registry rejected the request."
    ///      ],
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiWorkflowError {
        ///Machine-readable error code identifying the failure reason.
        pub code: ::std::string::String,
        ///Human-readable explanation of the failure. May include
        /// registry-specific details.
        pub message: ::std::string::String,
    }

    ///`RegistrarApiWorkflowLinks`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "self"
    ///  ],
    ///  "properties": {
    ///    "resource": {
    ///      "description": "URL to the domain resource.",
    ///      "examples": [
    ///        "/accounts/{account_id}/registrar/registrations/example.com"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "self": {
    ///      "description": "URL to this status resource.",
    ///      "examples": [
    ///        "/accounts/{account_id}/registrar/registrations/example.com/
    /// registration-status"
    ///      ],
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiWorkflowLinks {
        ///URL to the domain resource.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub resource: ::std::option::Option<::std::string::String>,
        ///URL to this status resource.
        #[serde(rename = "self")]
        pub self_: ::std::string::String,
    }

    ///Status of an async registration workflow.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Status of an async registration workflow.",
    ///  "type": "object",
    ///  "required": [
    ///    "completed",
    ///    "created_at",
    ///    "links",
    ///    "state",
    ///    "updated_at"
    ///  ],
    ///  "properties": {
    ///    "completed": {
    ///      "description": "Whether the workflow has reached a terminal state.
    /// `true` when\n`state` is `succeeded` or `failed`. `false` for
    /// `pending`,\n`in_progress`, `action_required`, and `blocked`.\n",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "context": {
    ///      "description": "Workflow-specific data for this workflow.\n\nThe
    /// workflow subject is identified by `context.domain_name`
    /// for\ndomain-centric workflows.\n",
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "created_at": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "error": {
    ///      "oneOf": [
    ///        {
    ///          "type": "null"
    ///        },
    ///        {
    ///          "allOf": [
    ///            {
    ///              "$ref": "#/components/schemas/registrar-api_workflow_error"
    ///            }
    ///          ]
    ///        }
    ///      ]
    ///    },
    ///    "links": {
    ///      "$ref": "#/components/schemas/registrar-api_workflow_links"
    ///    },
    ///    "state": {
    ///      "description": "Workflow lifecycle state.\n- `pending`: Workflow
    /// has been created but not yet started processing.\n- `in_progress`:
    /// Actively processing. Continue polling `links.self`.\n  The workflow has
    /// an internal deadline and will not remain in this\n  state
    /// indefinitely.\n- `action_required`: Paused — requires action by the user
    /// (not the\n  system). See `context.action` for what is needed. An
    /// automated\n  polling loop must break on this state; it will not resolve
    /// on its\n  own without user intervention.\n- `blocked`: The workflow
    /// cannot make progress due to a third party\n  such as the domain
    /// extension's registry or a losing registrar.\n  No user action will help.
    /// Continue polling — the block may resolve\n  when the third party
    /// responds.\n- `succeeded`: Terminal. The operation completed
    /// successfully.\n  `completed` will be `true`. For registrations,
    /// `context.registration`\n  contains the resulting registration
    /// resource.\n- `failed`: Terminal. The operation failed. `completed` will
    /// be `true`.\n  See `error.code` and `error.message` for the reason. Do
    /// not\n  auto-retry without user review.\n",
    ///      "examples": [
    ///        "in_progress"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "pending",
    ///        "in_progress",
    ///        "action_required",
    ///        "blocked",
    ///        "succeeded",
    ///        "failed"
    ///      ]
    ///    },
    ///    "updated_at": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiWorkflowStatus {
        ///Whether the workflow has reached a terminal state. `true` when
        ///`state` is `succeeded` or `failed`. `false` for `pending`,
        ///`in_progress`, `action_required`, and `blocked`.
        pub completed: bool,
        ///Workflow-specific data for this workflow.
        ///
        ///The workflow subject is identified by `context.domain_name` for
        ///domain-centric workflows.
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub context: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        pub created_at: ::chrono::DateTime<::chrono::offset::Utc>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error: ::std::option::Option<RegistrarApiWorkflowError>,
        pub links: RegistrarApiWorkflowLinks,
        ///Workflow lifecycle state.
        /// - `pending`: Workflow has been created but not yet started
        ///   processing.
        /// - `in_progress`: Actively processing. Continue polling `links.self`.
        ///  The workflow has an internal deadline and will not remain in this
        ///  state indefinitely.
        /// - `action_required`: Paused — requires action by the user (not the
        ///  system). See `context.action` for what is needed. An automated
        ///  polling loop must break on this state; it will not resolve on its
        ///  own without user intervention.
        /// - `blocked`: The workflow cannot make progress due to a third party
        ///  such as the domain extension's registry or a losing registrar.
        ///  No user action will help. Continue polling — the block may resolve
        ///  when the third party responds.
        /// - `succeeded`: Terminal. The operation completed successfully.
        ///  `completed` will be `true`. For registrations,
        /// `context.registration`  contains the resulting registration
        /// resource.
        /// - `failed`: Terminal. The operation failed. `completed` will be
        ///   `true`.
        ///  See `error.code` and `error.message` for the reason. Do not
        ///  auto-retry without user review.
        pub state: RegistrarApiWorkflowStatusState,
        pub updated_at: ::chrono::DateTime<::chrono::offset::Utc>,
    }

    ///`RegistrarApiWorkflowStatusResponseSingle`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/registrar-api_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/registrar-api_workflow_status"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RegistrarApiWorkflowStatusResponseSingle {
        pub errors: RegistrarApiMessages,
        pub messages: RegistrarApiMessages,
        pub result: RegistrarApiWorkflowStatus,
        ///Whether the API call was successful
        pub success: bool,
    }

    ///Workflow lifecycle state.
    /// - `pending`: Workflow has been created but not yet started processing.
    /// - `in_progress`: Actively processing. Continue polling `links.self`.
    ///  The workflow has an internal deadline and will not remain in this
    ///  state indefinitely.
    /// - `action_required`: Paused — requires action by the user (not the
    ///  system). See `context.action` for what is needed. An automated
    ///  polling loop must break on this state; it will not resolve on its
    ///  own without user intervention.
    /// - `blocked`: The workflow cannot make progress due to a third party
    ///  such as the domain extension's registry or a losing registrar.
    ///  No user action will help. Continue polling — the block may resolve
    ///  when the third party responds.
    /// - `succeeded`: Terminal. The operation completed successfully.
    ///  `completed` will be `true`. For registrations, `context.registration`
    ///  contains the resulting registration resource.
    /// - `failed`: Terminal. The operation failed. `completed` will be `true`.
    ///  See `error.code` and `error.message` for the reason. Do not
    ///  auto-retry without user review.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Workflow lifecycle state.\n- `pending`: Workflow has
    /// been created but not yet started processing.\n- `in_progress`: Actively
    /// processing. Continue polling `links.self`.\n  The workflow has an
    /// internal deadline and will not remain in this\n  state indefinitely.\n-
    /// `action_required`: Paused — requires action by the user (not the\n
    /// system). See `context.action` for what is needed. An automated\n
    /// polling loop must break on this state; it will not resolve on its\n  own
    /// without user intervention.\n- `blocked`: The workflow cannot make
    /// progress due to a third party\n  such as the domain extension's registry
    /// or a losing registrar.\n  No user action will help. Continue polling —
    /// the block may resolve\n  when the third party responds.\n- `succeeded`:
    /// Terminal. The operation completed successfully.\n  `completed` will be
    /// `true`. For registrations, `context.registration`\n  contains the
    /// resulting registration resource.\n- `failed`: Terminal. The operation
    /// failed. `completed` will be `true`.\n  See `error.code` and
    /// `error.message` for the reason. Do not\n  auto-retry without user
    /// review.\n",
    ///  "examples": [
    ///    "in_progress"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "pending",
    ///    "in_progress",
    ///    "action_required",
    ///    "blocked",
    ///    "succeeded",
    ///    "failed"
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
    pub enum RegistrarApiWorkflowStatusState {
        #[serde(rename = "pending")]
        Pending,
        #[serde(rename = "in_progress")]
        InProgress,
        #[serde(rename = "action_required")]
        ActionRequired,
        #[serde(rename = "blocked")]
        Blocked,
        #[serde(rename = "succeeded")]
        Succeeded,
        #[serde(rename = "failed")]
        Failed,
    }

    impl ::std::fmt::Display for RegistrarApiWorkflowStatusState {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Pending => f.write_str("pending"),
                Self::InProgress => f.write_str("in_progress"),
                Self::ActionRequired => f.write_str("action_required"),
                Self::Blocked => f.write_str("blocked"),
                Self::Succeeded => f.write_str("succeeded"),
                Self::Failed => f.write_str("failed"),
            }
        }
    }

    impl ::std::str::FromStr for RegistrarApiWorkflowStatusState {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "pending" => Ok(Self::Pending),
                "in_progress" => Ok(Self::InProgress),
                "action_required" => Ok(Self::ActionRequired),
                "blocked" => Ok(Self::Blocked),
                "succeeded" => Ok(Self::Succeeded),
                "failed" => Ok(Self::Failed),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for RegistrarApiWorkflowStatusState {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for RegistrarApiWorkflowStatusState {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for RegistrarApiWorkflowStatusState {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`RegistrarDomainRegistrationListCursor`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 256
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct RegistrarDomainRegistrationListCursor(::std::string::String);
    impl ::std::ops::Deref for RegistrarDomainRegistrationListCursor {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<RegistrarDomainRegistrationListCursor> for ::std::string::String {
        fn from(value: RegistrarDomainRegistrationListCursor) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for RegistrarDomainRegistrationListCursor {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 256usize {
                return Err("longer than 256 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for RegistrarDomainRegistrationListCursor {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for RegistrarDomainRegistrationListCursor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for RegistrarDomainRegistrationListCursor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for RegistrarDomainRegistrationListCursor {
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

    ///`RegistrarDomainRegistrationListDirection`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "asc",
    ///  "type": "string",
    ///  "enum": [
    ///    "asc",
    ///    "desc"
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
    pub enum RegistrarDomainRegistrationListDirection {
        #[serde(rename = "asc")]
        Asc,
        #[serde(rename = "desc")]
        Desc,
    }

    impl ::std::fmt::Display for RegistrarDomainRegistrationListDirection {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Asc => f.write_str("asc"),
                Self::Desc => f.write_str("desc"),
            }
        }
    }

    impl ::std::str::FromStr for RegistrarDomainRegistrationListDirection {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "asc" => Ok(Self::Asc),
                "desc" => Ok(Self::Desc),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for RegistrarDomainRegistrationListDirection {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for RegistrarDomainRegistrationListDirection {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for RegistrarDomainRegistrationListDirection {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for RegistrarDomainRegistrationListDirection {
        fn default() -> Self {
            RegistrarDomainRegistrationListDirection::Asc
        }
    }

    ///`RegistrarDomainRegistrationListSortBy`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "registry_created_at",
    ///  "type": "string",
    ///  "enum": [
    ///    "registry_created_at",
    ///    "registry_expires_at",
    ///    "name"
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
    pub enum RegistrarDomainRegistrationListSortBy {
        #[serde(rename = "registry_created_at")]
        RegistryCreatedAt,
        #[serde(rename = "registry_expires_at")]
        RegistryExpiresAt,
        #[serde(rename = "name")]
        Name,
    }

    impl ::std::fmt::Display for RegistrarDomainRegistrationListSortBy {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::RegistryCreatedAt => f.write_str("registry_created_at"),
                Self::RegistryExpiresAt => f.write_str("registry_expires_at"),
                Self::Name => f.write_str("name"),
            }
        }
    }

    impl ::std::str::FromStr for RegistrarDomainRegistrationListSortBy {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "registry_created_at" => Ok(Self::RegistryCreatedAt),
                "registry_expires_at" => Ok(Self::RegistryExpiresAt),
                "name" => Ok(Self::Name),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for RegistrarDomainRegistrationListSortBy {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for RegistrarDomainRegistrationListSortBy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for RegistrarDomainRegistrationListSortBy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for RegistrarDomainRegistrationListSortBy {
        fn default() -> Self {
            RegistrarDomainRegistrationListSortBy::RegistryCreatedAt
        }
    }

    ///`RegistrarDomainRegistrationUpdatePrefer`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "respond-async"
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
    pub enum RegistrarDomainRegistrationUpdatePrefer {
        #[serde(rename = "respond-async")]
        RespondAsync,
    }

    impl ::std::fmt::Display for RegistrarDomainRegistrationUpdatePrefer {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::RespondAsync => f.write_str("respond-async"),
            }
        }
    }

    impl ::std::str::FromStr for RegistrarDomainRegistrationUpdatePrefer {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "respond-async" => Ok(Self::RespondAsync),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for RegistrarDomainRegistrationUpdatePrefer {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for RegistrarDomainRegistrationUpdatePrefer {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for RegistrarDomainRegistrationUpdatePrefer {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`Zones0GetResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/zones_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/zones_zone"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct Zones0GetResponse {
        pub errors: ZonesMessages,
        pub messages: ZonesMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result: ::std::option::Option<ZonesZone>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`Zones0PatchBody`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "paused": true
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "paused": {
    ///      "$ref": "#/components/schemas/zones_paused"
    ///    },
    ///    "plan": {
    ///      "description": "(Deprecated) Please use the
    /// `/zones/{zone_id}/subscription` API\nto update a zone's plan. Changing
    /// this value will create/cancel\nassociated subscriptions. To view
    /// available plans for this zone,\nsee Zone Plans.\n",
    ///      "type": "object",
    ///      "properties": {
    ///        "id": {
    ///          "$ref": "#/components/schemas/zones_identifier"
    ///        }
    ///      }
    ///    },
    ///    "type": {
    ///      "description": "A full zone implies that DNS is hosted with
    /// Cloudflare. A partial\nzone is typically a partner-hosted zone or a
    /// CNAME setup. This\nparameter is only available to Enterprise customers
    /// or if it has\nbeen explicitly enabled on a zone.\n",
    ///      "examples": [
    ///        "full"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "full",
    ///        "partial",
    ///        "secondary",
    ///        "internal"
    ///      ]
    ///    },
    ///    "vanity_name_servers": {
    ///      "$ref": "#/components/schemas/zones_vanity_name_servers"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct Zones0PatchBody {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub paused: ::std::option::Option<ZonesPaused>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub plan: ::std::option::Option<Zones0PatchBodyPlan>,
        ///A full zone implies that DNS is hosted with Cloudflare. A partial
        ///zone is typically a partner-hosted zone or a CNAME setup. This
        ///parameter is only available to Enterprise customers or if it has
        ///been explicitly enabled on a zone.
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<Zones0PatchBodyType>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub vanity_name_servers: ::std::option::Option<ZonesVanityNameServers>,
    }

    impl ::std::default::Default for Zones0PatchBody {
        fn default() -> Self {
            Self {
                paused: Default::default(),
                plan: Default::default(),
                type_: Default::default(),
                vanity_name_servers: Default::default(),
            }
        }
    }

    ///(Deprecated) Please use the `/zones/{zone_id}/subscription` API
    ///to update a zone's plan. Changing this value will create/cancel
    ///associated subscriptions. To view available plans for this zone,
    ///see Zone Plans.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "(Deprecated) Please use the
    /// `/zones/{zone_id}/subscription` API\nto update a zone's plan. Changing
    /// this value will create/cancel\nassociated subscriptions. To view
    /// available plans for this zone,\nsee Zone Plans.\n",
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
    ///      "$ref": "#/components/schemas/zones_identifier"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct Zones0PatchBodyPlan {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<ZonesIdentifier>,
    }

    impl ::std::default::Default for Zones0PatchBodyPlan {
        fn default() -> Self {
            Self {
                id: Default::default(),
            }
        }
    }

    ///A full zone implies that DNS is hosted with Cloudflare. A partial
    ///zone is typically a partner-hosted zone or a CNAME setup. This
    ///parameter is only available to Enterprise customers or if it has
    ///been explicitly enabled on a zone.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A full zone implies that DNS is hosted with Cloudflare.
    /// A partial\nzone is typically a partner-hosted zone or a CNAME setup.
    /// This\nparameter is only available to Enterprise customers or if it
    /// has\nbeen explicitly enabled on a zone.\n",
    ///  "examples": [
    ///    "full"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "full",
    ///    "partial",
    ///    "secondary",
    ///    "internal"
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
    pub enum Zones0PatchBodyType {
        #[serde(rename = "full")]
        Full,
        #[serde(rename = "partial")]
        Partial,
        #[serde(rename = "secondary")]
        Secondary,
        #[serde(rename = "internal")]
        Internal,
    }

    impl ::std::fmt::Display for Zones0PatchBodyType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Full => f.write_str("full"),
                Self::Partial => f.write_str("partial"),
                Self::Secondary => f.write_str("secondary"),
                Self::Internal => f.write_str("internal"),
            }
        }
    }

    impl ::std::str::FromStr for Zones0PatchBodyType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "full" => Ok(Self::Full),
                "partial" => Ok(Self::Partial),
                "secondary" => Ok(Self::Secondary),
                "internal" => Ok(Self::Internal),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for Zones0PatchBodyType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for Zones0PatchBodyType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for Zones0PatchBodyType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`Zones0PatchResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/zones_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/zones_zone"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct Zones0PatchResponse {
        pub errors: ZonesMessages,
        pub messages: ZonesMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result: ::std::option::Option<ZonesZone>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`ZonesApiResponseCommon`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "$ref": "#/components/schemas/zones_messages"
    ///    },
    ///    "messages": {
    ///      "$ref": "#/components/schemas/zones_messages"
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesApiResponseCommon {
        pub errors: ZonesMessages,
        pub messages: ZonesMessages,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`ZonesApiResponseCommonFailure`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "errors",
    ///    "messages",
    ///    "result",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "errors": {
    ///      "examples": [
    ///        [
    ///          {
    ///            "code": 7003,
    ///            "message": "No route for the URI"
    ///          }
    ///        ]
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/zones_messages"
    ///        }
    ///      ],
    ///      "minLength": 1
    ///    },
    ///    "messages": {
    ///      "examples": [
    ///        []
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/zones_messages"
    ///        }
    ///      ]
    ///    },
    ///    "result": {
    ///      "type": [
    ///        "object",
    ///        "null"
    ///      ]
    ///    },
    ///    "success": {
    ///      "description": "Whether the API call was successful.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesApiResponseCommonFailure {
        pub errors: ::std::vec::Vec<ZonesApiResponseCommonFailureErrorsItem>,
        pub messages: ZonesMessages,
        pub result:
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`ZonesApiResponseCommonFailureErrorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesApiResponseCommonFailureErrorsItem {
        pub code: i64,
        pub message: ::std::string::String,
    }

    ///`ZonesApiResponseSingleId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/zones_api-response-common"
    ///    },
    ///    {
    ///      "properties": {
    ///        "result": {
    ///          "type": [
    ///            "object",
    ///            "null"
    ///          ],
    ///          "required": [
    ///            "id"
    ///          ],
    ///          "properties": {
    ///            "id": {
    ///              "$ref": "#/components/schemas/zones_identifier"
    ///            }
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesApiResponseSingleId {
        pub errors: ZonesMessages,
        pub messages: ZonesMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result: ::std::option::Option<ZonesApiResponseSingleIdResult>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`ZonesApiResponseSingleIdResult`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "id"
    ///  ],
    ///  "properties": {
    ///    "id": {
    ///      "$ref": "#/components/schemas/zones_identifier"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesApiResponseSingleIdResult {
        pub id: ZonesIdentifier,
    }

    ///An account Name. Optional filter operators can be provided to extend
    /// refine the search:
    ///  * `equal` (default)
    ///  * `not_equal`
    ///  * `starts_with`
    ///  * `ends_with`
    ///  * `contains`
    ///  * `starts_with_case_sensitive`
    ///  * `ends_with_case_sensitive`
    ///  * `contains_case_sensitive`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "An account Name. Optional filter operators can be
    /// provided to extend refine the search:\n  * `equal` (default)\n  *
    /// `not_equal`\n  * `starts_with`\n  * `ends_with`\n  * `contains`\n  *
    /// `starts_with_case_sensitive`\n  * `ends_with_case_sensitive`\n  *
    /// `contains_case_sensitive`\n",
    ///  "type": "string",
    ///  "maxLength": 253
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ZonesGetAccountName(::std::string::String);
    impl ::std::ops::Deref for ZonesGetAccountName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ZonesGetAccountName> for ::std::string::String {
        fn from(value: ZonesGetAccountName) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ZonesGetAccountName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 253usize {
                return Err("longer than 253 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesGetAccountName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesGetAccountName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesGetAccountName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ZonesGetAccountName {
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

    ///Direction to order zones.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Direction to order zones.",
    ///  "examples": [
    ///    "desc"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "asc",
    ///    "desc"
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
    pub enum ZonesGetDirection {
        #[serde(rename = "asc")]
        Asc,
        #[serde(rename = "desc")]
        Desc,
    }

    impl ::std::fmt::Display for ZonesGetDirection {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Asc => f.write_str("asc"),
                Self::Desc => f.write_str("desc"),
            }
        }
    }

    impl ::std::str::FromStr for ZonesGetDirection {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "asc" => Ok(Self::Asc),
                "desc" => Ok(Self::Desc),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesGetDirection {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesGetDirection {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesGetDirection {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Whether to match all search requirements or at least one (any).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Whether to match all search requirements or at least
    /// one (any).",
    ///  "default": "all",
    ///  "type": "string",
    ///  "enum": [
    ///    "any",
    ///    "all"
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
    pub enum ZonesGetMatch {
        #[serde(rename = "any")]
        Any,
        #[serde(rename = "all")]
        All,
    }

    impl ::std::fmt::Display for ZonesGetMatch {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Any => f.write_str("any"),
                Self::All => f.write_str("all"),
            }
        }
    }

    impl ::std::str::FromStr for ZonesGetMatch {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "any" => Ok(Self::Any),
                "all" => Ok(Self::All),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesGetMatch {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesGetMatch {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesGetMatch {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for ZonesGetMatch {
        fn default() -> Self {
            ZonesGetMatch::All
        }
    }

    ///A domain name. Optional filter operators can be provided to extend
    /// refine the search:
    ///  * `equal` (default)
    ///  * `not_equal`
    ///  * `starts_with`
    ///  * `ends_with`
    ///  * `contains`
    ///  * `starts_with_case_sensitive`
    ///  * `ends_with_case_sensitive`
    ///  * `contains_case_sensitive`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A domain name. Optional filter operators can be
    /// provided to extend refine the search:\n  * `equal` (default)\n  *
    /// `not_equal`\n  * `starts_with`\n  * `ends_with`\n  * `contains`\n  *
    /// `starts_with_case_sensitive`\n  * `ends_with_case_sensitive`\n  *
    /// `contains_case_sensitive`\n",
    ///  "type": "string",
    ///  "maxLength": 253
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ZonesGetName(::std::string::String);
    impl ::std::ops::Deref for ZonesGetName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ZonesGetName> for ::std::string::String {
        fn from(value: ZonesGetName) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ZonesGetName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 253usize {
                return Err("longer than 253 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesGetName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesGetName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesGetName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ZonesGetName {
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

    ///Field to order zones by.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Field to order zones by.",
    ///  "examples": [
    ///    "status"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "name",
    ///    "status",
    ///    "account.id",
    ///    "account.name",
    ///    "plan.id"
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
    pub enum ZonesGetOrder {
        #[serde(rename = "name")]
        Name,
        #[serde(rename = "status")]
        Status,
        #[serde(rename = "account.id")]
        AccountId,
        #[serde(rename = "account.name")]
        AccountName,
        #[serde(rename = "plan.id")]
        PlanId,
    }

    impl ::std::fmt::Display for ZonesGetOrder {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Name => f.write_str("name"),
                Self::Status => f.write_str("status"),
                Self::AccountId => f.write_str("account.id"),
                Self::AccountName => f.write_str("account.name"),
                Self::PlanId => f.write_str("plan.id"),
            }
        }
    }

    impl ::std::str::FromStr for ZonesGetOrder {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "name" => Ok(Self::Name),
                "status" => Ok(Self::Status),
                "account.id" => Ok(Self::AccountId),
                "account.name" => Ok(Self::AccountName),
                "plan.id" => Ok(Self::PlanId),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesGetOrder {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesGetOrder {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesGetOrder {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`ZonesGetResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/zones_api-response-common"
    ///    },
    ///    {
    ///      "properties": {
    ///        "result_info": {
    ///          "$ref": "#/components/schemas/zones_result_info"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "properties": {
    ///        "result": {
    ///          "type": "array",
    ///          "items": {
    ///            "$ref": "#/components/schemas/zones_zone"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesGetResponse {
        pub errors: ZonesMessages,
        pub messages: ZonesMessages,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub result: ::std::vec::Vec<ZonesZone>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result_info: ::std::option::Option<ZonesResultInfo>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///Specify a zone status to filter by.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specify a zone status to filter by.",
    ///  "type": "string",
    ///  "enum": [
    ///    "initializing",
    ///    "pending",
    ///    "active",
    ///    "moved"
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
    pub enum ZonesGetStatus {
        #[serde(rename = "initializing")]
        Initializing,
        #[serde(rename = "pending")]
        Pending,
        #[serde(rename = "active")]
        Active,
        #[serde(rename = "moved")]
        Moved,
    }

    impl ::std::fmt::Display for ZonesGetStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Initializing => f.write_str("initializing"),
                Self::Pending => f.write_str("pending"),
                Self::Active => f.write_str("active"),
                Self::Moved => f.write_str("moved"),
            }
        }
    }

    impl ::std::str::FromStr for ZonesGetStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "initializing" => Ok(Self::Initializing),
                "pending" => Ok(Self::Pending),
                "active" => Ok(Self::Active),
                "moved" => Ok(Self::Moved),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesGetStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesGetStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesGetStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`ZonesGetTypeItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "full",
    ///    "partial",
    ///    "secondary",
    ///    "internal"
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
    pub enum ZonesGetTypeItem {
        #[serde(rename = "full")]
        Full,
        #[serde(rename = "partial")]
        Partial,
        #[serde(rename = "secondary")]
        Secondary,
        #[serde(rename = "internal")]
        Internal,
    }

    impl ::std::fmt::Display for ZonesGetTypeItem {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Full => f.write_str("full"),
                Self::Partial => f.write_str("partial"),
                Self::Secondary => f.write_str("secondary"),
                Self::Internal => f.write_str("internal"),
            }
        }
    }

    impl ::std::str::FromStr for ZonesGetTypeItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "full" => Ok(Self::Full),
                "partial" => Ok(Self::Partial),
                "secondary" => Ok(Self::Secondary),
                "internal" => Ok(Self::Internal),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesGetTypeItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesGetTypeItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesGetTypeItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Identifier
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Identifier",
    ///  "examples": [
    ///    "023e105f4ecef8ad9ca31a8372d0c353"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ZonesIdentifier(::std::string::String);
    impl ::std::ops::Deref for ZonesIdentifier {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ZonesIdentifier> for ::std::string::String {
        fn from(value: ZonesIdentifier) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ZonesIdentifier {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesIdentifier {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ZonesIdentifier {
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

    ///`ZonesMessages`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    []
    ///  ],
    ///  "type": "array",
    ///  "items": {
    ///    "type": "object",
    ///    "uniqueItems": true,
    ///    "required": [
    ///      "code",
    ///      "message"
    ///    ],
    ///    "properties": {
    ///      "code": {
    ///        "type": "integer",
    ///        "minimum": 1000.0
    ///      },
    ///      "message": {
    ///        "type": "string"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct ZonesMessages(pub ::std::vec::Vec<ZonesMessagesItem>);
    impl ::std::ops::Deref for ZonesMessages {
        type Target = ::std::vec::Vec<ZonesMessagesItem>;
        fn deref(&self) -> &::std::vec::Vec<ZonesMessagesItem> {
            &self.0
        }
    }

    impl ::std::convert::From<ZonesMessages> for ::std::vec::Vec<ZonesMessagesItem> {
        fn from(value: ZonesMessages) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::vec::Vec<ZonesMessagesItem>> for ZonesMessages {
        fn from(value: ::std::vec::Vec<ZonesMessagesItem>) -> Self {
            Self(value)
        }
    }

    ///`ZonesMessagesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "uniqueItems": true,
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "minimum": 1000.0
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesMessagesItem {
        pub code: i64,
        pub message: ::std::string::String,
    }

    ///The domain name. Per [RFC 1035](https://datatracker.ietf.org/doc/html/rfc1035#section-2.3.4) the overall zone name can be up to 253 characters, with each segment ("label") not exceeding 63 characters.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The domain name. Per [RFC 1035](https://datatracker.ietf.org/doc/html/rfc1035#section-2.3.4) the overall zone name can be up to 253 characters, with each segment (\"label\") not exceeding 63 characters.",
    ///  "examples": [
    ///    "example.com"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 253,
    ///  "pattern": "^([a-zA-Z0-9][\\-a-zA-Z0-9]*\\.)+[\\-a-zA-Z0-9]{2,20}$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ZonesName(::std::string::String);
    impl ::std::ops::Deref for ZonesName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ZonesName> for ::std::string::String {
        fn from(value: ZonesName) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ZonesName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 253usize {
                return Err("longer than 253 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^([a-zA-Z0-9][\\-a-zA-Z0-9]*\\.)+[\\-a-zA-Z0-9]{2,20}$")
                        .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^([a-zA-Z0-9][\\-a-zA-Z0-9]*\\.)+[\\-a-zA-Z0-9]{2,20}$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ZonesName {
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

    ///Indicates whether the zone is only using Cloudflare DNS services. A
    ///true value means the zone will not receive security or performance
    ///benefits.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Indicates whether the zone is only using Cloudflare DNS
    /// services. A\ntrue value means the zone will not receive security or
    /// performance\nbenefits.\n",
    ///  "default": false,
    ///  "type": "boolean"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct ZonesPaused(pub bool);
    impl ::std::ops::Deref for ZonesPaused {
        type Target = bool;
        fn deref(&self) -> &bool {
            &self.0
        }
    }

    impl ::std::convert::From<ZonesPaused> for bool {
        fn from(value: ZonesPaused) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<bool> for ZonesPaused {
        fn from(value: bool) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for ZonesPaused {
        type Err = <bool as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesPaused {
        type Error = <bool as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for ZonesPaused {
        type Error = <bool as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for ZonesPaused {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///`ZonesPostBody`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "account",
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "account": {
    ///      "type": "object",
    ///      "properties": {
    ///        "id": {
    ///          "$ref": "#/components/schemas/zones_identifier"
    ///        }
    ///      }
    ///    },
    ///    "name": {
    ///      "$ref": "#/components/schemas/zones_name"
    ///    },
    ///    "type": {
    ///      "$ref": "#/components/schemas/zones_type"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesPostBody {
        pub account: ZonesPostBodyAccount,
        pub name: ZonesName,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<ZonesType>,
    }

    ///`ZonesPostBodyAccount`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
    ///      "$ref": "#/components/schemas/zones_identifier"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesPostBodyAccount {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<ZonesIdentifier>,
    }

    impl ::std::default::Default for ZonesPostBodyAccount {
        fn default() -> Self {
            Self {
                id: Default::default(),
            }
        }
    }

    ///`ZonesPostResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/zones_api-response-common"
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "result": {
    ///          "$ref": "#/components/schemas/zones_zone"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesPostResponse {
        pub errors: ZonesMessages,
        pub messages: ZonesMessages,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result: ::std::option::Option<ZonesZone>,
        ///Whether the API call was successful.
        pub success: bool,
    }

    ///`ZonesResultInfo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "count": {
    ///      "description": "Total number of results for the requested
    /// service.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "page": {
    ///      "description": "Current page within paginated list of results.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "per_page": {
    ///      "description": "Number of results per page of results.",
    ///      "examples": [
    ///        20
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_count": {
    ///      "description": "Total results available without any search
    /// parameters.",
    ///      "examples": [
    ///        2000
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "total_pages": {
    ///      "description": "Total number of pages",
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesResultInfo {
        ///Total number of results for the requested service.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub count: ::std::option::Option<f64>,
        ///Current page within paginated list of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub page: ::std::option::Option<f64>,
        ///Number of results per page of results.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub per_page: ::std::option::Option<f64>,
        ///Total results available without any search parameters.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_count: ::std::option::Option<f64>,
        ///Total number of pages
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_pages: ::std::option::Option<f64>,
    }

    impl ::std::default::Default for ZonesResultInfo {
        fn default() -> Self {
            Self {
                count: Default::default(),
                page: Default::default(),
                per_page: Default::default(),
                total_count: Default::default(),
                total_pages: Default::default(),
            }
        }
    }

    ///A full zone implies that DNS is hosted with Cloudflare. A partial zone
    /// is typically a partner-hosted zone or a CNAME setup.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A full zone implies that DNS is hosted with Cloudflare. A partial zone is\ntypically a partner-hosted zone or a CNAME setup.\n",
    ///  "default": "full",
    ///  "examples": [
    ///    "full"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "full",
    ///    "partial",
    ///    "secondary",
    ///    "internal"
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
    pub enum ZonesType {
        #[serde(rename = "full")]
        Full,
        #[serde(rename = "partial")]
        Partial,
        #[serde(rename = "secondary")]
        Secondary,
        #[serde(rename = "internal")]
        Internal,
    }

    impl ::std::fmt::Display for ZonesType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Full => f.write_str("full"),
                Self::Partial => f.write_str("partial"),
                Self::Secondary => f.write_str("secondary"),
                Self::Internal => f.write_str("internal"),
            }
        }
    }

    impl ::std::str::FromStr for ZonesType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "full" => Ok(Self::Full),
                "partial" => Ok(Self::Partial),
                "secondary" => Ok(Self::Secondary),
                "internal" => Ok(Self::Internal),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::default::Default for ZonesType {
        fn default() -> Self {
            ZonesType::Full
        }
    }

    ///An array of domains used for custom name servers. This is only
    ///available for Business and Enterprise plans.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "An array of domains used for custom name servers. This
    /// is only\navailable for Business and Enterprise plans.",
    ///  "default": [],
    ///  "examples": [
    ///    [
    ///      "ns1.example.com",
    ///      "ns2.example.com"
    ///    ]
    ///  ],
    ///  "type": "array",
    ///  "items": {
    ///    "type": "string",
    ///    "format": "hostname",
    ///    "maxLength": 253
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct ZonesVanityNameServers(pub ::std::vec::Vec<::std::string::String>);
    impl ::std::ops::Deref for ZonesVanityNameServers {
        type Target = ::std::vec::Vec<::std::string::String>;
        fn deref(&self) -> &::std::vec::Vec<::std::string::String> {
            &self.0
        }
    }

    impl ::std::convert::From<ZonesVanityNameServers> for ::std::vec::Vec<::std::string::String> {
        fn from(value: ZonesVanityNameServers) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::vec::Vec<::std::string::String>> for ZonesVanityNameServers {
        fn from(value: ::std::vec::Vec<::std::string::String>) -> Self {
            Self(value)
        }
    }

    ///`ZonesZone`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "account",
    ///    "activated_on",
    ///    "created_on",
    ///    "development_mode",
    ///    "id",
    ///    "meta",
    ///    "modified_on",
    ///    "name",
    ///    "name_servers",
    ///    "original_dnshost",
    ///    "original_name_servers",
    ///    "original_registrar",
    ///    "owner",
    ///    "plan"
    ///  ],
    ///  "properties": {
    ///    "account": {
    ///      "description": "The account the zone belongs to.",
    ///      "type": "object",
    ///      "properties": {
    ///        "id": {
    ///          "$ref": "#/components/schemas/zones_identifier"
    ///        },
    ///        "name": {
    ///          "description": "The name of the account.",
    ///          "examples": [
    ///            "Example Account Name"
    ///          ],
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    "activated_on": {
    ///      "description": "The last time proof of ownership was detected and
    /// the zone was made\nactive.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "2014-01-02T00:01:00.12345Z"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "format": "date-time"
    ///    },
    ///    "cname_suffix": {
    ///      "description": "Allows the customer to use a custom apex.\n*Tenants
    /// Only Configuration*.",
    ///      "examples": [
    ///        "cdn.cloudflare.com"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "created_on": {
    ///      "description": "When the zone was created.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "2014-01-01T05:20:00.12345Z"
    ///      ],
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "development_mode": {
    ///      "description": "The interval (in seconds) from when development
    /// mode expires\n(positive integer) or last expired (negative integer) for
    /// the\ndomain. If development mode has never been enabled, this value is
    /// 0.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        7200
    ///      ],
    ///      "type": "number"
    ///    },
    ///    "id": {
    ///      "$ref": "#/components/schemas/zones_identifier"
    ///    },
    ///    "meta": {
    ///      "description": "Metadata about the zone.",
    ///      "type": "object",
    ///      "properties": {
    ///        "cdn_only": {
    ///          "description": "The zone is only configured for CDN.",
    ///          "examples": [
    ///            true
    ///          ],
    ///          "type": "boolean"
    ///        },
    ///        "custom_certificate_quota": {
    ///          "description": "Number of Custom Certificates the zone can
    /// have.",
    ///          "examples": [
    ///            1
    ///          ],
    ///          "type": "integer"
    ///        },
    ///        "dns_only": {
    ///          "description": "The zone is only configured for DNS.",
    ///          "examples": [
    ///            true
    ///          ],
    ///          "type": "boolean"
    ///        },
    ///        "foundation_dns": {
    ///          "description": "The zone is setup with Foundation DNS.",
    ///          "examples": [
    ///            true
    ///          ],
    ///          "type": "boolean"
    ///        },
    ///        "page_rule_quota": {
    ///          "description": "Number of Page Rules a zone can have.",
    ///          "examples": [
    ///            100
    ///          ],
    ///          "type": "integer"
    ///        },
    ///        "phishing_detected": {
    ///          "description": "The zone has been flagged for phishing.",
    ///          "examples": [
    ///            false
    ///          ],
    ///          "type": "boolean"
    ///        },
    ///        "step": {
    ///          "examples": [
    ///            2
    ///          ],
    ///          "type": "integer"
    ///        }
    ///      }
    ///    },
    ///    "modified_on": {
    ///      "description": "When the zone was last modified.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "2014-01-01T05:20:00.12345Z"
    ///      ],
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "name": {
    ///      "description": "The domain name. Per [RFC 1035](https://datatracker.ietf.org/doc/html/rfc1035#section-2.3.4) the overall zone name can be up to 253 characters, with each segment (\"label\") not exceeding 63 characters.",
    ///      "examples": [
    ///        "example.com"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 253,
    ///      "pattern": "^([a-zA-Z0-9][\\-a-zA-Z0-9]*\\.)+[\\-a-zA-Z0-9]{2,20}$"
    ///    },
    ///    "name_servers": {
    ///      "description": "The name servers Cloudflare assigns to a zone.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        [
    ///          "bob.ns.cloudflare.com",
    ///          "lola.ns.cloudflare.com"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "format": "hostname"
    ///      }
    ///    },
    ///    "original_dnshost": {
    ///      "description": "DNS host at the time of switching to Cloudflare.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "NameCheap"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ],
    ///      "maxLength": 50
    ///    },
    ///    "original_name_servers": {
    ///      "description": "Original name servers before moving to
    /// Cloudflare.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        [
    ///          "ns1.originaldnshost.com",
    ///          "ns2.originaldnshost.com"
    ///        ]
    ///      ],
    ///      "type": [
    ///        "array",
    ///        "null"
    ///      ],
    ///      "items": {
    ///        "type": "string",
    ///        "format": "hostname"
    ///      }
    ///    },
    ///    "original_registrar": {
    ///      "description": "Registrar for the domain at the time of switching
    /// to Cloudflare.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "GoDaddy"
    ///      ],
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "owner": {
    ///      "description": "The owner of the zone.",
    ///      "type": "object",
    ///      "properties": {
    ///        "id": {
    ///          "$ref": "#/components/schemas/zones_identifier"
    ///        },
    ///        "name": {
    ///          "description": "Name of the owner.",
    ///          "examples": [
    ///            "Example Org"
    ///          ],
    ///          "type": "string"
    ///        },
    ///        "type": {
    ///          "description": "The type of owner.",
    ///          "examples": [
    ///            "organization"
    ///          ],
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    "paused": {
    ///      "$ref": "#/components/schemas/zones_paused"
    ///    },
    ///    "permissions": {
    ///      "description": "Legacy permissions based on legacy user membership
    /// information.",
    ///      "deprecated": true,
    ///      "type": "array",
    ///      "items": {
    ///        "examples": [
    ///          "#worker:read"
    ///        ],
    ///        "type": "string"
    ///      },
    ///      "x-stainless-deprecation-message": "This has been replaced by
    /// Account memberships."
    ///    },
    ///    "plan": {
    ///      "description": "A Zones subscription information.",
    ///      "deprecated": true,
    ///      "properties": {
    ///        "can_subscribe": {
    ///          "description": "States if the subscription can be activated.",
    ///          "examples": [
    ///            false
    ///          ],
    ///          "type": "boolean"
    ///        },
    ///        "currency": {
    ///          "description": "The denomination of the customer.",
    ///          "examples": [
    ///            "USD"
    ///          ],
    ///          "type": "string"
    ///        },
    ///        "externally_managed": {
    ///          "description": "If this Zone is managed by another company.",
    ///          "examples": [
    ///            false
    ///          ],
    ///          "type": "boolean"
    ///        },
    ///        "frequency": {
    ///          "description": "How often the customer is billed.",
    ///          "examples": [
    ///            "monthly"
    ///          ],
    ///          "type": "string"
    ///        },
    ///        "id": {
    ///          "$ref": "#/components/schemas/zones_identifier"
    ///        },
    ///        "is_subscribed": {
    ///          "description": "States if the subscription active.",
    ///          "examples": [
    ///            false
    ///          ],
    ///          "type": "boolean"
    ///        },
    ///        "legacy_discount": {
    ///          "description": "If the legacy discount applies to this Zone.",
    ///          "examples": [
    ///            false
    ///          ],
    ///          "type": "boolean"
    ///        },
    ///        "legacy_id": {
    ///          "description": "The legacy name of the plan.",
    ///          "examples": [
    ///            "free"
    ///          ],
    ///          "type": "string"
    ///        },
    ///        "name": {
    ///          "description": "Name of the owner.",
    ///          "examples": [
    ///            "Example Org"
    ///          ],
    ///          "type": "string"
    ///        },
    ///        "price": {
    ///          "description": "How much the customer is paying.",
    ///          "examples": [
    ///            10.99
    ///          ],
    ///          "type": "number"
    ///        }
    ///      },
    ///      "x-stainless-deprecation-message": "Please use the `/zones/{zone_id}/subscription` API\nto update a zone's plan. Changing this value will create/cancel\nassociated subscriptions. To view available plans for this zone,\nsee [Zone Plans](https://developers.cloudflare.com/api/resources/zones/subresources/plans/)."
    ///    },
    ///    "status": {
    ///      "description": "The zone status on Cloudflare.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "active"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "initializing",
    ///        "pending",
    ///        "active",
    ///        "moved"
    ///      ]
    ///    },
    ///    "tenant": {
    ///      "description": "The root organizational unit that this zone belongs
    /// to (such as a tenant or organization).",
    ///      "properties": {
    ///        "id": {
    ///          "$ref": "#/components/schemas/zones_identifier"
    ///        },
    ///        "name": {
    ///          "description": "The name of the Tenant account.",
    ///          "examples": [
    ///            "Example Account Name"
    ///          ],
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    "tenant_unit": {
    ///      "description": "The immediate parent organizational unit that this
    /// zone belongs to (such as under a tenant or sub-organization).",
    ///      "properties": {
    ///        "id": {
    ///          "$ref": "#/components/schemas/zones_identifier"
    ///        }
    ///      }
    ///    },
    ///    "type": {
    ///      "$ref": "#/components/schemas/zones_type"
    ///    },
    ///    "vanity_name_servers": {
    ///      "description": "An array of domains used for custom name servers.
    /// This is only available for Business and Enterprise plans.",
    ///      "default": [],
    ///      "examples": [
    ///        [
    ///          "ns1.example.com",
    ///          "ns2.example.com"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "format": "hostname",
    ///        "maxLength": 253
    ///      }
    ///    },
    ///    "verification_key": {
    ///      "description": "Verification key for partial zone setup.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "284344499-1084221259"
    ///      ],
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesZone {
        pub account: ZonesZoneAccount,
        ///The last time proof of ownership was detected and the zone was made
        ///active.
        pub activated_on: ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
        ///Allows the customer to use a custom apex.
        ///*Tenants Only Configuration*.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub cname_suffix: ::std::option::Option<::std::string::String>,
        ///When the zone was created.
        pub created_on: ::chrono::DateTime<::chrono::offset::Utc>,
        ///The interval (in seconds) from when development mode expires
        ///(positive integer) or last expired (negative integer) for the
        ///domain. If development mode has never been enabled, this value is 0.
        pub development_mode: f64,
        pub id: ZonesIdentifier,
        pub meta: ZonesZoneMeta,
        ///When the zone was last modified.
        pub modified_on: ::chrono::DateTime<::chrono::offset::Utc>,
        ///The domain name. Per [RFC 1035](https://datatracker.ietf.org/doc/html/rfc1035#section-2.3.4) the overall zone name can be up to 253 characters, with each segment ("label") not exceeding 63 characters.
        pub name: ZonesZoneName,
        ///The name servers Cloudflare assigns to a zone.
        pub name_servers: ::std::vec::Vec<::std::string::String>,
        ///DNS host at the time of switching to Cloudflare.
        pub original_dnshost: ::std::option::Option<ZonesZoneOriginalDnshost>,
        ///Original name servers before moving to Cloudflare.
        pub original_name_servers: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
        ///Registrar for the domain at the time of switching to Cloudflare.
        pub original_registrar: ::std::option::Option<::std::string::String>,
        pub owner: ZonesZoneOwner,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub paused: ::std::option::Option<ZonesPaused>,
        ///Legacy permissions based on legacy user membership information.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub permissions: ::std::vec::Vec<::std::string::String>,
        pub plan: ZonesZonePlan,
        ///The zone status on Cloudflare.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub status: ::std::option::Option<ZonesZoneStatus>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tenant: ::std::option::Option<ZonesZoneTenant>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tenant_unit: ::std::option::Option<ZonesZoneTenantUnit>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<ZonesType>,
        ///An array of domains used for custom name servers. This is only
        /// available for Business and Enterprise plans.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub vanity_name_servers: ::std::vec::Vec<::std::string::String>,
        ///Verification key for partial zone setup.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub verification_key: ::std::option::Option<::std::string::String>,
    }

    ///The account the zone belongs to.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The account the zone belongs to.",
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
    ///      "$ref": "#/components/schemas/zones_identifier"
    ///    },
    ///    "name": {
    ///      "description": "The name of the account.",
    ///      "examples": [
    ///        "Example Account Name"
    ///      ],
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesZoneAccount {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<ZonesIdentifier>,
        ///The name of the account.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for ZonesZoneAccount {
        fn default() -> Self {
            Self {
                id: Default::default(),
                name: Default::default(),
            }
        }
    }

    ///Metadata about the zone.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Metadata about the zone.",
    ///  "type": "object",
    ///  "properties": {
    ///    "cdn_only": {
    ///      "description": "The zone is only configured for CDN.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "custom_certificate_quota": {
    ///      "description": "Number of Custom Certificates the zone can have.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer"
    ///    },
    ///    "dns_only": {
    ///      "description": "The zone is only configured for DNS.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "foundation_dns": {
    ///      "description": "The zone is setup with Foundation DNS.",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "page_rule_quota": {
    ///      "description": "Number of Page Rules a zone can have.",
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "integer"
    ///    },
    ///    "phishing_detected": {
    ///      "description": "The zone has been flagged for phishing.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "step": {
    ///      "examples": [
    ///        2
    ///      ],
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesZoneMeta {
        ///The zone is only configured for CDN.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub cdn_only: ::std::option::Option<bool>,
        ///Number of Custom Certificates the zone can have.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub custom_certificate_quota: ::std::option::Option<i64>,
        ///The zone is only configured for DNS.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub dns_only: ::std::option::Option<bool>,
        ///The zone is setup with Foundation DNS.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub foundation_dns: ::std::option::Option<bool>,
        ///Number of Page Rules a zone can have.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub page_rule_quota: ::std::option::Option<i64>,
        ///The zone has been flagged for phishing.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub phishing_detected: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub step: ::std::option::Option<i64>,
    }

    impl ::std::default::Default for ZonesZoneMeta {
        fn default() -> Self {
            Self {
                cdn_only: Default::default(),
                custom_certificate_quota: Default::default(),
                dns_only: Default::default(),
                foundation_dns: Default::default(),
                page_rule_quota: Default::default(),
                phishing_detected: Default::default(),
                step: Default::default(),
            }
        }
    }

    ///The domain name. Per [RFC 1035](https://datatracker.ietf.org/doc/html/rfc1035#section-2.3.4) the overall zone name can be up to 253 characters, with each segment ("label") not exceeding 63 characters.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The domain name. Per [RFC 1035](https://datatracker.ietf.org/doc/html/rfc1035#section-2.3.4) the overall zone name can be up to 253 characters, with each segment (\"label\") not exceeding 63 characters.",
    ///  "examples": [
    ///    "example.com"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 253,
    ///  "pattern": "^([a-zA-Z0-9][\\-a-zA-Z0-9]*\\.)+[\\-a-zA-Z0-9]{2,20}$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ZonesZoneName(::std::string::String);
    impl ::std::ops::Deref for ZonesZoneName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ZonesZoneName> for ::std::string::String {
        fn from(value: ZonesZoneName) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ZonesZoneName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 253usize {
                return Err("longer than 253 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^([a-zA-Z0-9][\\-a-zA-Z0-9]*\\.)+[\\-a-zA-Z0-9]{2,20}$")
                        .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^([a-zA-Z0-9][\\-a-zA-Z0-9]*\\.)+[\\-a-zA-Z0-9]{2,20}$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesZoneName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesZoneName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesZoneName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ZonesZoneName {
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

    ///DNS host at the time of switching to Cloudflare.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS host at the time of switching to Cloudflare.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "NameCheap"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 50
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ZonesZoneOriginalDnshost(::std::string::String);
    impl ::std::ops::Deref for ZonesZoneOriginalDnshost {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ZonesZoneOriginalDnshost> for ::std::string::String {
        fn from(value: ZonesZoneOriginalDnshost) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ZonesZoneOriginalDnshost {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 50usize {
                return Err("longer than 50 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesZoneOriginalDnshost {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesZoneOriginalDnshost {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesZoneOriginalDnshost {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ZonesZoneOriginalDnshost {
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

    ///The owner of the zone.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The owner of the zone.",
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
    ///      "$ref": "#/components/schemas/zones_identifier"
    ///    },
    ///    "name": {
    ///      "description": "Name of the owner.",
    ///      "examples": [
    ///        "Example Org"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "type": {
    ///      "description": "The type of owner.",
    ///      "examples": [
    ///        "organization"
    ///      ],
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesZoneOwner {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<ZonesIdentifier>,
        ///Name of the owner.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        ///The type of owner.
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for ZonesZoneOwner {
        fn default() -> Self {
            Self {
                id: Default::default(),
                name: Default::default(),
                type_: Default::default(),
            }
        }
    }

    ///A Zones subscription information.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A Zones subscription information.",
    ///  "deprecated": true,
    ///  "properties": {
    ///    "can_subscribe": {
    ///      "description": "States if the subscription can be activated.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "currency": {
    ///      "description": "The denomination of the customer.",
    ///      "examples": [
    ///        "USD"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "externally_managed": {
    ///      "description": "If this Zone is managed by another company.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "frequency": {
    ///      "description": "How often the customer is billed.",
    ///      "examples": [
    ///        "monthly"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "id": {
    ///      "$ref": "#/components/schemas/zones_identifier"
    ///    },
    ///    "is_subscribed": {
    ///      "description": "States if the subscription active.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "legacy_discount": {
    ///      "description": "If the legacy discount applies to this Zone.",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "legacy_id": {
    ///      "description": "The legacy name of the plan.",
    ///      "examples": [
    ///        "free"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "description": "Name of the owner.",
    ///      "examples": [
    ///        "Example Org"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "price": {
    ///      "description": "How much the customer is paying.",
    ///      "examples": [
    ///        10.99
    ///      ],
    ///      "type": "number"
    ///    }
    ///  },
    ///  "x-stainless-deprecation-message": "Please use the `/zones/{zone_id}/subscription` API\nto update a zone's plan. Changing this value will create/cancel\nassociated subscriptions. To view available plans for this zone,\nsee [Zone Plans](https://developers.cloudflare.com/api/resources/zones/subresources/plans/)."
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesZonePlan {
        ///States if the subscription can be activated.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub can_subscribe: ::std::option::Option<bool>,
        ///The denomination of the customer.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub currency: ::std::option::Option<::std::string::String>,
        ///If this Zone is managed by another company.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub externally_managed: ::std::option::Option<bool>,
        ///How often the customer is billed.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub frequency: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<ZonesIdentifier>,
        ///States if the subscription active.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub is_subscribed: ::std::option::Option<bool>,
        ///If the legacy discount applies to this Zone.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub legacy_discount: ::std::option::Option<bool>,
        ///The legacy name of the plan.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub legacy_id: ::std::option::Option<::std::string::String>,
        ///Name of the owner.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        ///How much the customer is paying.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price: ::std::option::Option<f64>,
    }

    impl ::std::default::Default for ZonesZonePlan {
        fn default() -> Self {
            Self {
                can_subscribe: Default::default(),
                currency: Default::default(),
                externally_managed: Default::default(),
                frequency: Default::default(),
                id: Default::default(),
                is_subscribed: Default::default(),
                legacy_discount: Default::default(),
                legacy_id: Default::default(),
                name: Default::default(),
                price: Default::default(),
            }
        }
    }

    ///The zone status on Cloudflare.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The zone status on Cloudflare.",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "active"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "initializing",
    ///    "pending",
    ///    "active",
    ///    "moved"
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
    pub enum ZonesZoneStatus {
        #[serde(rename = "initializing")]
        Initializing,
        #[serde(rename = "pending")]
        Pending,
        #[serde(rename = "active")]
        Active,
        #[serde(rename = "moved")]
        Moved,
    }

    impl ::std::fmt::Display for ZonesZoneStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Initializing => f.write_str("initializing"),
                Self::Pending => f.write_str("pending"),
                Self::Active => f.write_str("active"),
                Self::Moved => f.write_str("moved"),
            }
        }
    }

    impl ::std::str::FromStr for ZonesZoneStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "initializing" => Ok(Self::Initializing),
                "pending" => Ok(Self::Pending),
                "active" => Ok(Self::Active),
                "moved" => Ok(Self::Moved),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for ZonesZoneStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ZonesZoneStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ZonesZoneStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///The root organizational unit that this zone belongs to (such as a tenant
    /// or organization).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The root organizational unit that this zone belongs to
    /// (such as a tenant or organization).",
    ///  "properties": {
    ///    "id": {
    ///      "$ref": "#/components/schemas/zones_identifier"
    ///    },
    ///    "name": {
    ///      "description": "The name of the Tenant account.",
    ///      "examples": [
    ///        "Example Account Name"
    ///      ],
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesZoneTenant {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<ZonesIdentifier>,
        ///The name of the Tenant account.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
    }

    impl ::std::default::Default for ZonesZoneTenant {
        fn default() -> Self {
            Self {
                id: Default::default(),
                name: Default::default(),
            }
        }
    }

    ///The immediate parent organizational unit that this zone belongs to (such
    /// as under a tenant or sub-organization).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The immediate parent organizational unit that this zone
    /// belongs to (such as under a tenant or sub-organization).",
    ///  "properties": {
    ///    "id": {
    ///      "$ref": "#/components/schemas/zones_identifier"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ZonesZoneTenantUnit {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<ZonesIdentifier>,
    }

    impl ::std::default::Default for ZonesZoneTenantUnit {
        fn default() -> Self {
            Self {
                id: Default::default(),
            }
        }
    }

    /// Generation of default values for serde.
    pub mod defaults {
        pub(super) fn default_bool<const V: bool>() -> bool {
            V
        }

        pub(super) fn default_u64<T, const V: u64>() -> T
        where
            T: ::std::convert::TryFrom<u64>,
            <T as ::std::convert::TryFrom<u64>>::Error: ::std::fmt::Debug,
        {
            T::try_from(V).unwrap()
        }

        pub(super) fn pages_deployment_config_values_request_usage_model(
        ) -> super::PagesDeploymentConfigValuesRequestUsageModel {
            super::PagesDeploymentConfigValuesRequestUsageModel::Standard
        }

        pub(super) fn registrar_api_registration_create_request_privacy_mode(
        ) -> super::RegistrarApiRegistrationCreateRequestPrivacyMode {
            super::RegistrarApiRegistrationCreateRequestPrivacyMode::Redaction
        }
    }
}

#[derive(Clone, Debug)]
///Client for Cloudflare API (trimmed for web-agency)
///
///Version: 4.0.0
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
        "4.0.0"
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
    ///List Accounts
    ///
    ///List all accounts you have ownership or verified access to.
    ///
    ///Sends a `GET` request to `/accounts`
    pub async fn accounts_list_accounts<'a>(
        &'a self,
        direction: Option<types::AccountsListAccountsDirection>,
        name: Option<&'a str>,
        page: Option<f64>,
        per_page: Option<f64>,
    ) -> Result<
        ResponseValue<types::IamResponseCollectionAccounts>,
        Error<types::IamApiResponseCommonFailure>,
    > {
        let url = format!("{}/accounts", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("direction", &direction))
            .query(&progenitor_client::QueryParam::new("name", &name))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .query(&progenitor_client::QueryParam::new("per_page", &per_page))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "accounts_list_accounts",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Create an account
    ///
    ///Create an account (only available for tenant admins at this time)
    ///
    ///Sends a `POST` request to `/accounts`
    ///
    ///Arguments:
    /// - `body`: Parameters for account creation
    pub async fn account_creation<'a>(
        &'a self,
        body: &'a types::IamCreateAccount,
    ) -> Result<
        ResponseValue<types::IamResponseSingleAccount>,
        Error<types::IamApiResponseCommonFailure>,
    > {
        let url = format!("{}/accounts", self.baseurl,);
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
            operation_id: "account_creation",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Account Details
    ///
    ///Get information about a specific account that you are a member of.
    ///
    ///Sends a `GET` request to `/accounts/{account_id}`
    pub async fn accounts_account_details<'a>(
        &'a self,
        account_id: &'a types::IamAccountIdentifier,
    ) -> Result<
        ResponseValue<types::IamResponseSingleAccount>,
        Error<types::IamApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}",
            self.baseurl,
            encode_path(&account_id.to_string()),
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
            operation_id: "accounts_account_details",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update Account
    ///
    ///Update an existing account.
    ///
    ///Sends a `PUT` request to `/accounts/{account_id}`
    pub async fn accounts_update_account<'a>(
        &'a self,
        account_id: &'a types::IamAccountIdentifier,
        body: &'a types::IamComponentsSchemasAccount,
    ) -> Result<
        ResponseValue<types::IamResponseSingleAccount>,
        Error<types::IamApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}",
            self.baseurl,
            encode_path(&account_id.to_string()),
        );
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
            operation_id: "accounts_update_account",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete a specific account
    ///
    ///Delete a specific account (only available for tenant admins at this
    /// time). This is a permanent operation that will delete any zones or other
    /// resources under the account
    ///
    ///Sends a `DELETE` request to `/accounts/{account_id}`
    pub async fn account_deletion<'a>(
        &'a self,
        account_id: &'a str,
    ) -> Result<
        ResponseValue<types::IamApiResponseSingleId>,
        Error<types::IamApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}",
            self.baseurl,
            encode_path(&account_id.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .delete(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "account_deletion",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get projects
    ///
    ///Fetch a list of all user projects.
    ///
    ///Sends a `GET` request to `/accounts/{account_id}/pages/projects`
    pub async fn pages_project_get_projects<'a>(
        &'a self,
        account_id: &'a types::PagesIdentifier,
        page: Option<i64>,
        per_page: Option<i64>,
    ) -> Result<
        ResponseValue<types::PagesProjectGetProjectsResponse>,
        Error<types::PagesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/pages/projects",
            self.baseurl,
            encode_path(&account_id.to_string()),
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
            .query(&progenitor_client::QueryParam::new("page", &page))
            .query(&progenitor_client::QueryParam::new("per_page", &per_page))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "pages_project_get_projects",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Create project
    ///
    ///Create a new project.
    ///
    ///Sends a `POST` request to `/accounts/{account_id}/pages/projects`
    pub async fn pages_project_create_project<'a>(
        &'a self,
        account_id: &'a types::PagesIdentifier,
        body: &'a types::PagesProjectCreateProjectBody,
    ) -> Result<
        ResponseValue<types::PagesProjectCreateProjectResponse>,
        Error<types::PagesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/pages/projects",
            self.baseurl,
            encode_path(&account_id.to_string()),
        );
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
            operation_id: "pages_project_create_project",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get project
    ///
    ///Fetch a project by name.
    ///
    ///Sends a `GET` request to
    /// `/accounts/{account_id}/pages/projects/{project_name}`
    pub async fn pages_project_get_project<'a>(
        &'a self,
        account_id: &'a types::PagesIdentifier,
        project_name: &'a types::PagesProjectName,
    ) -> Result<
        ResponseValue<types::PagesProjectGetProjectResponse>,
        Error<types::PagesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/pages/projects/{}",
            self.baseurl,
            encode_path(&account_id.to_string()),
            encode_path(&project_name.to_string()),
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
            operation_id: "pages_project_get_project",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete project
    ///
    ///Delete a project by name.
    ///
    ///Sends a `DELETE` request to
    /// `/accounts/{account_id}/pages/projects/{project_name}`
    pub async fn pages_project_delete_project<'a>(
        &'a self,
        account_id: &'a types::PagesIdentifier,
        project_name: &'a types::PagesProjectName,
    ) -> Result<
        ResponseValue<types::PagesProjectDeleteProjectResponse>,
        Error<types::PagesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/pages/projects/{}",
            self.baseurl,
            encode_path(&account_id.to_string()),
            encode_path(&project_name.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .delete(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "pages_project_delete_project",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update project
    ///
    ///Set new attributes for an existing project. Modify environment
    /// variables. To delete an environment variable, set the key to null.
    ///
    ///Sends a `PATCH` request to
    /// `/accounts/{account_id}/pages/projects/{project_name}`
    pub async fn pages_project_update_project<'a>(
        &'a self,
        account_id: &'a types::PagesIdentifier,
        project_name: &'a types::PagesProjectName,
        body: &'a types::PagesProjectUpdateProjectBody,
    ) -> Result<
        ResponseValue<types::PagesProjectUpdateProjectResponse>,
        Error<types::PagesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/pages/projects/{}",
            self.baseurl,
            encode_path(&account_id.to_string()),
            encode_path(&project_name.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .patch(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "pages_project_update_project",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get domains
    ///
    ///Fetch a list of all domains associated with a Pages project.
    ///
    ///Sends a `GET` request to
    /// `/accounts/{account_id}/pages/projects/{project_name}/domains`
    pub async fn pages_domains_get_domains<'a>(
        &'a self,
        account_id: &'a types::PagesIdentifier,
        project_name: &'a types::PagesProjectName,
    ) -> Result<
        ResponseValue<types::PagesDomainsGetDomainsResponse>,
        Error<types::PagesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/pages/projects/{}/domains",
            self.baseurl,
            encode_path(&account_id.to_string()),
            encode_path(&project_name.to_string()),
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
            operation_id: "pages_domains_get_domains",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Add domain
    ///
    ///Add a new domain for the Pages project.
    ///
    ///Sends a `POST` request to
    /// `/accounts/{account_id}/pages/projects/{project_name}/domains`
    pub async fn pages_domains_add_domain<'a>(
        &'a self,
        account_id: &'a types::PagesIdentifier,
        project_name: &'a types::PagesProjectName,
        body: &'a types::PagesDomainsAddDomainBody,
    ) -> Result<
        ResponseValue<types::PagesDomainsAddDomainResponse>,
        Error<types::PagesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/pages/projects/{}/domains",
            self.baseurl,
            encode_path(&account_id.to_string()),
            encode_path(&project_name.to_string()),
        );
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
            operation_id: "pages_domains_add_domain",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get domain
    ///
    ///Fetch a single domain.
    ///
    ///Sends a `GET` request to
    /// `/accounts/{account_id}/pages/projects/{project_name}/domains/
    /// {domain_name}`
    pub async fn pages_domains_get_domain<'a>(
        &'a self,
        account_id: &'a types::PagesIdentifier,
        project_name: &'a types::PagesProjectName,
        domain_name: &'a types::PagesDomainName,
    ) -> Result<
        ResponseValue<types::PagesDomainsGetDomainResponse>,
        Error<types::PagesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/pages/projects/{}/domains/{}",
            self.baseurl,
            encode_path(&account_id.to_string()),
            encode_path(&project_name.to_string()),
            encode_path(&domain_name.to_string()),
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
            operation_id: "pages_domains_get_domain",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete domain
    ///
    ///Delete a Pages project's domain.
    ///
    ///Sends a `DELETE` request to
    /// `/accounts/{account_id}/pages/projects/{project_name}/domains/
    /// {domain_name}`
    pub async fn pages_domains_delete_domain<'a>(
        &'a self,
        account_id: &'a types::PagesIdentifier,
        project_name: &'a types::PagesProjectName,
        domain_name: &'a types::PagesDomainName,
    ) -> Result<
        ResponseValue<types::PagesDomainsDeleteDomainResponse>,
        Error<types::PagesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/pages/projects/{}/domains/{}",
            self.baseurl,
            encode_path(&account_id.to_string()),
            encode_path(&project_name.to_string()),
            encode_path(&domain_name.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .delete(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "pages_domains_delete_domain",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Patch domain
    ///
    ///Retry the validation status of a single domain.
    ///
    ///Sends a `PATCH` request to
    /// `/accounts/{account_id}/pages/projects/{project_name}/domains/
    /// {domain_name}`
    pub async fn pages_domains_patch_domain<'a>(
        &'a self,
        account_id: &'a types::PagesIdentifier,
        project_name: &'a types::PagesProjectName,
        domain_name: &'a types::PagesDomainName,
    ) -> Result<
        ResponseValue<types::PagesDomainsPatchDomainResponse>,
        Error<types::PagesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/pages/projects/{}/domains/{}",
            self.baseurl,
            encode_path(&account_id.to_string()),
            encode_path(&project_name.to_string()),
            encode_path(&domain_name.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .patch(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "pages_domains_patch_domain",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Check domain availability
    ///
    ///Performs real-time, authoritative availability checks directly against
    /// domain registries. Use this endpoint to verify a domain is available
    /// before attempting registration via `POST /registrations`.
    ///
    ///**Important:** Unlike the Search endpoint, these results are
    /// authoritative and reflect current registry status. Always check
    /// availability immediately before registration as domain status can
    /// change rapidly.
    ///
    ///**Note:** This endpoint uses POST to accept a list of domains in the
    /// request body. It is a read-only operation — it does not create,
    /// modify, or reserve any domains.
    ///
    ///### Extension support
    ///
    ///Only domains on extensions supported for programmatic registration by
    /// this API can be registered. If you check a domain on an unsupported
    /// extension, the response will include `registrable: false` with a
    /// `reason` field explaining why:
    ///
    /// - `extension_not_supported_via_api` — Cloudflare Registrar supports this
    ///   extension
    ///  in the dashboard, but it is not yet available for programmatic
    /// registration via  this API. Register via `https://dash.cloudflare.com/{account_id}/domains/registrations` instead.
    /// - `extension_not_supported` — This extension is not supported by
    ///   Cloudflare
    ///  Registrar.
    /// - `extension_disallows_registration` — The extension's registry has
    ///   temporarily
    ///  or permanently frozen new registrations. No registrar can register
    /// domains on  this extension at this time.
    /// - `domain_premium` — The domain is premium priced. Premium registration
    ///   is not
    ///  currently supported by this API.
    /// - `domain_unavailable` — The domain is already registered, reserved, or
    ///   otherwise
    ///  not available for registration on a supported extension.
    ///
    ///The `reason` field is only present when `registrable` is `false`.
    ///
    ///### Behavior
    /// - Maximum 20 domains per request
    /// - Pricing is only returned for domains where `registrable: true`
    /// - Results are not cached; each request queries the registry
    ///
    ///### Workflow
    /// 1. Call this endpoint with domains the user wants to register.
    /// 2. For each domain where `registrable: true`, present pricing to the
    ///    user.
    /// 3. If `tier: premium`, note that premium registration is not currently
    ///   supported by this API and do not proceed to `POST /registrations`.
    /// 4. Proceed to `POST /registrations` only for supported non-premium
    ///    domains.
    ///
    ///
    ///Sends a `POST` request to
    /// `/accounts/{account_id}/registrar/domain-check`
    ///
    ///Arguments:
    /// - `account_id`: Cloudflare account ID. Required for all Registrar API
    ///   operations.
    /// - `body`: List of fully qualified domain names (FQDNs) to check. Each
    ///   domain must include the extension (e.g., "example.com", not
    ///   "example").
    pub async fn registrar_domain_discovery_check<'a>(
        &'a self,
        account_id: &'a types::RegistrarApiIdentifier,
        body: &'a types::RegistrarApiDomainCheckRequest,
    ) -> Result<
        ResponseValue<types::RegistrarApiDomainCheckResponse>,
        Error<types::RegistrarApiApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/registrar/domain-check",
            self.baseurl,
            encode_path(&account_id.to_string()),
        );
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
            operation_id: "registrar_domain_discovery_check",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///List Registrations
    ///
    ///Returns a paginated list of domain registrations owned by the account.
    ///
    ///This endpoint uses cursor-based pagination. Results are ordered by
    /// registration date by default. To fetch the next page, pass the
    /// `cursor` value from the `result_info` object in the response as the
    /// `cursor` query parameter in your next request. An empty `cursor`
    /// string indicates there are no more pages.
    ///
    ///
    ///Sends a `GET` request to
    /// `/accounts/{account_id}/registrar/registrations`
    ///
    ///Arguments:
    /// - `account_id`: Cloudflare account ID.
    /// - `cursor`: Opaque token from a previous response's
    ///   `result_info.cursor`.
    ///Pass this value to fetch the next page of results. Omit (or
    ///pass an empty string) for the first page.
    ///
    /// - `direction`: Sort direction for results. Defaults to ascending order.
    ///
    /// - `per_page`: Number of items to return per page.
    /// - `sort_by`: Column to sort results by. Defaults to registration date
    ///(`registry_created_at`) when omitted.
    pub async fn registrar_domain_registration_list<'a>(
        &'a self,
        account_id: &'a types::RegistrarApiIdentifier,
        cursor: Option<&'a types::RegistrarDomainRegistrationListCursor>,
        direction: Option<types::RegistrarDomainRegistrationListDirection>,
        per_page: Option<::std::num::NonZeroU64>,
        sort_by: Option<types::RegistrarDomainRegistrationListSortBy>,
    ) -> Result<
        ResponseValue<types::RegistrarApiRegistrationResponseCollection>,
        Error<types::RegistrarApiApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/registrar/registrations",
            self.baseurl,
            encode_path(&account_id.to_string()),
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
            .query(&progenitor_client::QueryParam::new("cursor", &cursor))
            .query(&progenitor_client::QueryParam::new("direction", &direction))
            .query(&progenitor_client::QueryParam::new("per_page", &per_page))
            .query(&progenitor_client::QueryParam::new("sort_by", &sort_by))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "registrar_domain_registration_list",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Create Registration
    ///
    ///Starts a domain registration workflow. This is a billable operation —
    /// successful registration charges the account's default payment
    /// method. All successful domain registrations are non-refundable —
    /// once the workflow completes with `state: succeeded`, the charge
    /// cannot be reversed.
    ///
    ///### Prerequisites
    /// - The account must have a billing profile with a valid default payment
    ///   method.
    ///  Set this up at `https://dash.cloudflare.com/{account_id}/billing/payment-info`.
    /// - The account must not already be at the maximum supported domain limit.
    ///  A single account may own up to 100 domains in total across
    /// registrations  created through either the dashboard or this API.
    /// - The domain must be on a supported extension for programmatic
    ///   registration.
    /// - Use `POST /domain-check` immediately before calling this endpoint to
    ///   confirm
    ///  real-time availability and pricing.
    ///
    ///### Supported extensions
    ///In this API, "extension" means the full registrable suffix after the
    /// domain label. For example, in `example.co.uk`, the extension is
    /// `co.uk`.
    ///
    ///Programmatic registration is currently supported for:
    ///
    ///`com`, `org`, `net`, `app`, `dev`, `cc`, `xyz`, `info`, `cloud`,
    /// `studio`, `live`, `link`, `pro`, `tech`, `fyi`, `shop`, `online`,
    /// `tools`, `run`, `games`, `build`, `systems`, `world`, `news`,
    /// `site`, `network`, `chat`, `space`, `family`, `page`, `life`,
    /// `group`, `email`, `solutions`, `day`, `blog`, `ing`, `icu`,
    /// `academy`, `today`
    ///
    ///Cloudflare Registrar supports 400+ extensions in the dashboard.
    /// Extensions not listed above can still be registered at
    ///`https://dash.cloudflare.com/{account_id}/domains/registrations`.
    ///
    ///### Express mode
    ///The only required field is `domain_name`. If `contacts` is omitted, the
    /// system uses the account's default address book entry as the
    /// registrant. If no default exists and no contact is provided, the
    /// request fails. Set up a default address book entry and accept the
    /// required agreement at `https://dash.cloudflare.com/{account_id}/domains/registrations`.
    ///
    ///### Defaults
    /// - `years`: defaults to the extension's minimum registration period (1
    ///   year for
    ///  most extensions, but varies — for example, `.ai` (if supported)
    /// requires a minimum of 2 years).
    /// - `auto_renew`: defaults to `false`. Setting it to `true` is an explicit
    ///  opt-in authorizing Cloudflare to charge the account's default payment
    ///  method up to 30 days before domain expiry to renew the registration.
    ///  Renewal pricing may change over time based on registry pricing.
    /// - `privacy_mode`: defaults to `redaction`.
    ///
    ///### Premium domains
    ///Premium domain registration is not currently supported by this API.
    ///If `POST /domain-check` returns `tier: premium`, do not call this
    ///endpoint for that domain.
    ///
    ///### Response behavior
    ///By default, the server holds the connection for a bounded,
    /// server-defined amount of time while the registration completes. Most
    /// registrations finish within this window and return `201 Created`
    /// with a completed workflow status.
    ///
    ///If the registration is still processing after this synchronous wait
    /// window, the server returns `202 Accepted`. Poll the URL in
    /// `links.self` to track progress.
    ///
    ///To skip the wait and receive an immediate `202`, send `Prefer:
    /// respond-async`.
    ///
    ///
    ///Sends a `POST` request to
    /// `/accounts/{account_id}/registrar/registrations`
    ///
    ///Arguments:
    /// - `account_id`: Cloudflare account ID. Required for all Registrar API
    ///   operations.
    /// - `prefer`: Set to `respond-async` to receive an immediate `202
    ///   Accepted` without
    ///waiting for the operation to complete (RFC 7240).
    ///
    ///The header may be combined with other preferences using standard
    ///comma-separated syntax.
    ///
    /// - `body`
    pub async fn registrar_domain_registration_create<'a>(
        &'a self,
        account_id: &'a types::RegistrarApiIdentifier,
        prefer: Option<&'a str>,
        body: &'a types::RegistrarApiRegistrationCreateRequest,
    ) -> Result<
        ResponseValue<types::RegistrarApiWorkflowStatusResponseSingle>,
        Error<types::RegistrarApiApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/registrar/registrations",
            self.baseurl,
            encode_path(&account_id.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        if let Some(value) = prefer {
            header_map.append("Prefer", value.to_string().try_into()?);
        }

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
            operation_id: "registrar_domain_registration_create",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            201u16 => ResponseValue::from_response(response).await,
            202u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get Registration
    ///
    ///Returns the current state of a domain registration.
    ///
    ///This is the canonical read endpoint for a domain you own. It returns
    ///the full registration resource including current settings and
    /// expiration. When the registration resource is ready, both
    /// `created_at` and `expires_at` are present in the response.
    ///
    ///
    ///Sends a `GET` request to
    /// `/accounts/{account_id}/registrar/registrations/{domain_name}`
    ///
    ///Arguments:
    /// - `account_id`: Cloudflare account ID.
    /// - `domain_name`: Domain name to retrieve.
    pub async fn registrar_domain_registration_get<'a>(
        &'a self,
        account_id: &'a types::RegistrarApiIdentifier,
        domain_name: &'a types::RegistrarApiDomainName,
    ) -> Result<
        ResponseValue<types::RegistrarApiRegistrationResponseSingle>,
        Error<types::RegistrarApiApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/registrar/registrations/{}",
            self.baseurl,
            encode_path(&account_id.to_string()),
            encode_path(&domain_name.to_string()),
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
            operation_id: "registrar_domain_registration_get",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update Registration
    ///
    ///Updates an existing domain registration.
    ///
    ///By default, the server holds the connection for a bounded,
    /// server-defined amount of time while the update completes. Most
    /// updates finish within this window and return `200 OK` with a
    /// completed workflow status.
    ///
    ///If the update is still processing after this synchronous wait window,
    /// the server returns `202 Accepted`. Poll the URL in `links.self` to
    /// track progress.
    ///
    ///To skip the wait and receive an immediate `202`, send `Prefer:
    /// respond-async`.
    ///
    ///This endpoint currently supports updating `auto_renew` only.
    ///
    ///
    ///Sends a `PATCH` request to
    /// `/accounts/{account_id}/registrar/registrations/{domain_name}`
    ///
    ///Arguments:
    /// - `account_id`: Cloudflare account ID.
    /// - `domain_name`: Domain name to update.
    /// - `prefer`: Set to `respond-async` to receive an immediate `202
    ///   Accepted` without
    ///waiting for the operation to complete (RFC 7240).
    ///
    /// - `body`
    pub async fn registrar_domain_registration_update<'a>(
        &'a self,
        account_id: &'a types::RegistrarApiIdentifier,
        domain_name: &'a types::RegistrarApiDomainName,
        prefer: Option<types::RegistrarDomainRegistrationUpdatePrefer>,
        body: &'a types::RegistrarApiRegistrationUpdateRequest,
    ) -> Result<
        ResponseValue<types::RegistrarApiWorkflowStatusResponseSingle>,
        Error<types::RegistrarApiApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/accounts/{}/registrar/registrations/{}",
            self.baseurl,
            encode_path(&account_id.to_string()),
            encode_path(&domain_name.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        if let Some(value) = prefer {
            header_map.append("Prefer", value.to_string().try_into()?);
        }

        #[allow(unused_mut)]
        let mut request = self
            .client
            .patch(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "registrar_domain_registration_update",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            202u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///List Zones
    ///
    ///Lists, searches, sorts, and filters your zones. Listing zones across
    /// more than 500 accounts is currently not allowed.
    ///
    ///
    ///Sends a `GET` request to `/zones`
    pub async fn zones_get<'a>(
        &'a self,
        account_id: Option<&'a str>,
        account_name: Option<&'a types::ZonesGetAccountName>,
        direction: Option<types::ZonesGetDirection>,
        match_: Option<types::ZonesGetMatch>,
        name: Option<&'a types::ZonesGetName>,
        order: Option<types::ZonesGetOrder>,
        page: Option<f64>,
        per_page: Option<f64>,
        status: Option<types::ZonesGetStatus>,
        type_: Option<&'a ::std::vec::Vec<types::ZonesGetTypeItem>>,
    ) -> Result<ResponseValue<types::ZonesGetResponse>, Error<types::ZonesApiResponseCommonFailure>>
    {
        let url = format!("{}/zones", self.baseurl,);
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
                "account.id",
                &account_id,
            ))
            .query(&progenitor_client::QueryParam::new(
                "account.name",
                &account_name,
            ))
            .query(&progenitor_client::QueryParam::new("direction", &direction))
            .query(&progenitor_client::QueryParam::new("match", &match_))
            .query(&progenitor_client::QueryParam::new("name", &name))
            .query(&progenitor_client::QueryParam::new("order", &order))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .query(&progenitor_client::QueryParam::new("per_page", &per_page))
            .query(&progenitor_client::QueryParam::new("status", &status))
            .query(&progenitor_client::QueryParam::new("type", &type_))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "zones_get",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Create Zone
    ///
    ///Sends a `POST` request to `/zones`
    pub async fn zones_post<'a>(
        &'a self,
        body: &'a types::ZonesPostBody,
    ) -> Result<ResponseValue<types::ZonesPostResponse>, Error<types::ZonesApiResponseCommonFailure>>
    {
        let url = format!("{}/zones", self.baseurl,);
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
            operation_id: "zones_post",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Zone Details
    ///
    ///Sends a `GET` request to `/zones/{zone_id}`
    pub async fn zones_0_get<'a>(
        &'a self,
        zone_id: &'a types::ZonesIdentifier,
    ) -> Result<ResponseValue<types::Zones0GetResponse>, Error<types::ZonesApiResponseCommonFailure>>
    {
        let url = format!(
            "{}/zones/{}",
            self.baseurl,
            encode_path(&zone_id.to_string()),
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
            operation_id: "zones_0_get",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete Zone
    ///
    ///Deletes an existing zone.
    ///
    ///Sends a `DELETE` request to `/zones/{zone_id}`
    pub async fn zones_0_delete<'a>(
        &'a self,
        zone_id: &'a types::ZonesIdentifier,
        body: &'a ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    ) -> Result<
        ResponseValue<types::ZonesApiResponseSingleId>,
        Error<types::ZonesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/zones/{}",
            self.baseurl,
            encode_path(&zone_id.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .delete(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "zones_0_delete",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Edit Zone
    ///
    ///Edits a zone. Only one zone property can be changed at a time.
    ///
    ///Sends a `PATCH` request to `/zones/{zone_id}`
    pub async fn zones_0_patch<'a>(
        &'a self,
        zone_id: &'a types::ZonesIdentifier,
        body: &'a types::Zones0PatchBody,
    ) -> Result<
        ResponseValue<types::Zones0PatchResponse>,
        Error<types::ZonesApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/zones/{}",
            self.baseurl,
            encode_path(&zone_id.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .patch(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "zones_0_patch",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///List DNS Records
    ///
    ///List, search, sort, and filter a zones' DNS records.
    ///
    ///Sends a `GET` request to `/zones/{zone_id}/dns_records`
    pub async fn dns_records_for_a_zone_list_dns_records<'a>(
        &'a self,
        zone_id: &'a types::DnsRecordsIdentifier,
        comment: Option<&'a str>,
        comment_absent: Option<&'a str>,
        comment_contains: Option<&'a str>,
        comment_endswith: Option<&'a str>,
        comment_exact: Option<&'a str>,
        comment_present: Option<&'a str>,
        comment_startswith: Option<&'a str>,
        content: Option<&'a str>,
        content_contains: Option<&'a str>,
        content_endswith: Option<&'a str>,
        content_exact: Option<&'a str>,
        content_startswith: Option<&'a str>,
        direction: Option<types::DnsRecordsDirection>,
        match_: Option<types::DnsRecordsMatch>,
        name: Option<&'a str>,
        name_contains: Option<&'a str>,
        name_endswith: Option<&'a str>,
        name_exact: Option<&'a str>,
        name_startswith: Option<&'a str>,
        order: Option<types::DnsRecordsOrder>,
        page: Option<&'a types::DnsRecordsPage>,
        per_page: Option<&'a types::DnsRecordsPerPage>,
        proxied: Option<&'a types::DnsRecordsProxied>,
        search: Option<&'a types::DnsRecordsSearch>,
        tag: Option<&'a str>,
        tag_absent: Option<&'a str>,
        tag_contains: Option<&'a str>,
        tag_endswith: Option<&'a str>,
        tag_exact: Option<&'a str>,
        tag_present: Option<&'a str>,
        tag_startswith: Option<&'a str>,
        tag_match: Option<types::DnsRecordsTagMatch>,
        type_: Option<types::DnsRecordsType>,
    ) -> Result<
        ResponseValue<types::DnsRecordsDnsResponseCollection>,
        Error<types::DnsRecordsForAZoneListDnsRecordsResponse>,
    > {
        let url = format!(
            "{}/zones/{}/dns_records",
            self.baseurl,
            encode_path(&zone_id.to_string()),
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
            .query(&progenitor_client::QueryParam::new("comment", &comment))
            .query(&progenitor_client::QueryParam::new(
                "comment.absent",
                &comment_absent,
            ))
            .query(&progenitor_client::QueryParam::new(
                "comment.contains",
                &comment_contains,
            ))
            .query(&progenitor_client::QueryParam::new(
                "comment.endswith",
                &comment_endswith,
            ))
            .query(&progenitor_client::QueryParam::new(
                "comment.exact",
                &comment_exact,
            ))
            .query(&progenitor_client::QueryParam::new(
                "comment.present",
                &comment_present,
            ))
            .query(&progenitor_client::QueryParam::new(
                "comment.startswith",
                &comment_startswith,
            ))
            .query(&progenitor_client::QueryParam::new("content", &content))
            .query(&progenitor_client::QueryParam::new(
                "content.contains",
                &content_contains,
            ))
            .query(&progenitor_client::QueryParam::new(
                "content.endswith",
                &content_endswith,
            ))
            .query(&progenitor_client::QueryParam::new(
                "content.exact",
                &content_exact,
            ))
            .query(&progenitor_client::QueryParam::new(
                "content.startswith",
                &content_startswith,
            ))
            .query(&progenitor_client::QueryParam::new("direction", &direction))
            .query(&progenitor_client::QueryParam::new("match", &match_))
            .query(&progenitor_client::QueryParam::new("name", &name))
            .query(&progenitor_client::QueryParam::new(
                "name.contains",
                &name_contains,
            ))
            .query(&progenitor_client::QueryParam::new(
                "name.endswith",
                &name_endswith,
            ))
            .query(&progenitor_client::QueryParam::new(
                "name.exact",
                &name_exact,
            ))
            .query(&progenitor_client::QueryParam::new(
                "name.startswith",
                &name_startswith,
            ))
            .query(&progenitor_client::QueryParam::new("order", &order))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .query(&progenitor_client::QueryParam::new("per_page", &per_page))
            .query(&progenitor_client::QueryParam::new("proxied", &proxied))
            .query(&progenitor_client::QueryParam::new("search", &search))
            .query(&progenitor_client::QueryParam::new("tag", &tag))
            .query(&progenitor_client::QueryParam::new(
                "tag.absent",
                &tag_absent,
            ))
            .query(&progenitor_client::QueryParam::new(
                "tag.contains",
                &tag_contains,
            ))
            .query(&progenitor_client::QueryParam::new(
                "tag.endswith",
                &tag_endswith,
            ))
            .query(&progenitor_client::QueryParam::new("tag.exact", &tag_exact))
            .query(&progenitor_client::QueryParam::new(
                "tag.present",
                &tag_present,
            ))
            .query(&progenitor_client::QueryParam::new(
                "tag.startswith",
                &tag_startswith,
            ))
            .query(&progenitor_client::QueryParam::new("tag_match", &tag_match))
            .query(&progenitor_client::QueryParam::new("type", &type_))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "dns_records_for_a_zone_list_dns_records",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Create DNS Record
    ///
    ///Create a new DNS record for a zone.
    ///
    ///Notes:
    /// - A/AAAA records cannot exist on the same name as CNAME records.
    /// - NS records cannot exist on the same name as any other record type.
    /// - Domain names are always represented in Punycode, even if Unicode
    ///  characters were used when creating the record.
    ///
    ///
    ///Sends a `POST` request to `/zones/{zone_id}/dns_records`
    pub async fn dns_records_for_a_zone_create_dns_record<'a>(
        &'a self,
        zone_id: &'a types::DnsRecordsIdentifier,
        body: &'a types::DnsRecordsDnsRecordPost,
    ) -> Result<
        ResponseValue<types::DnsRecordsDnsResponseSingle>,
        Error<types::DnsRecordsForAZoneCreateDnsRecordResponse>,
    > {
        let url = format!(
            "{}/zones/{}/dns_records",
            self.baseurl,
            encode_path(&zone_id.to_string()),
        );
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
            operation_id: "dns_records_for_a_zone_create_dns_record",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///DNS Record Details
    ///
    ///Sends a `GET` request to `/zones/{zone_id}/dns_records/{dns_record_id}`
    pub async fn dns_records_for_a_zone_dns_record_details<'a>(
        &'a self,
        zone_id: &'a types::DnsRecordsIdentifier,
        dns_record_id: &'a types::DnsRecordsIdentifier,
    ) -> Result<
        ResponseValue<types::DnsRecordsDnsResponseSingle>,
        Error<types::DnsRecordsForAZoneDnsRecordDetailsResponse>,
    > {
        let url = format!(
            "{}/zones/{}/dns_records/{}",
            self.baseurl,
            encode_path(&zone_id.to_string()),
            encode_path(&dns_record_id.to_string()),
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
            operation_id: "dns_records_for_a_zone_dns_record_details",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Overwrite DNS Record
    ///
    ///Overwrite an existing DNS record.
    ///
    ///Notes:
    /// - A/AAAA records cannot exist on the same name as CNAME records.
    /// - NS records cannot exist on the same name as any other record type.
    /// - Domain names are always represented in Punycode, even if Unicode
    ///  characters were used when creating the record.
    ///
    ///
    ///Sends a `PUT` request to `/zones/{zone_id}/dns_records/{dns_record_id}`
    pub async fn dns_records_for_a_zone_update_dns_record<'a>(
        &'a self,
        zone_id: &'a types::DnsRecordsIdentifier,
        dns_record_id: &'a types::DnsRecordsIdentifier,
        body: &'a types::DnsRecordsDnsRecordPost,
    ) -> Result<
        ResponseValue<types::DnsRecordsDnsResponseSingle>,
        Error<types::DnsRecordsForAZoneUpdateDnsRecordResponse>,
    > {
        let url = format!(
            "{}/zones/{}/dns_records/{}",
            self.baseurl,
            encode_path(&zone_id.to_string()),
            encode_path(&dns_record_id.to_string()),
        );
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
            operation_id: "dns_records_for_a_zone_update_dns_record",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete DNS Record
    ///
    ///Sends a `DELETE` request to
    /// `/zones/{zone_id}/dns_records/{dns_record_id}`
    pub async fn dns_records_for_a_zone_delete_dns_record<'a>(
        &'a self,
        zone_id: &'a types::DnsRecordsIdentifier,
        dns_record_id: &'a types::DnsRecordsIdentifier,
        body: &'a ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    ) -> Result<
        ResponseValue<types::DnsRecordsForAZoneDeleteDnsRecordResponse>,
        Error<types::DnsRecordsForAZoneDeleteDnsRecordResponse>,
    > {
        let url = format!(
            "{}/zones/{}/dns_records/{}",
            self.baseurl,
            encode_path(&zone_id.to_string()),
            encode_path(&dns_record_id.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .delete(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "dns_records_for_a_zone_delete_dns_record",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update DNS Record
    ///
    ///Update an existing DNS record.
    ///
    ///Notes:
    /// - A/AAAA records cannot exist on the same name as CNAME records.
    /// - NS records cannot exist on the same name as any other record type.
    /// - Domain names are always represented in Punycode, even if Unicode
    ///  characters were used when creating the record.
    ///
    ///
    ///Sends a `PATCH` request to
    /// `/zones/{zone_id}/dns_records/{dns_record_id}`
    pub async fn dns_records_for_a_zone_patch_dns_record<'a>(
        &'a self,
        zone_id: &'a types::DnsRecordsIdentifier,
        dns_record_id: &'a types::DnsRecordsIdentifier,
        body: &'a types::DnsRecordsDnsRecordPatch,
    ) -> Result<
        ResponseValue<types::DnsRecordsDnsResponseSingle>,
        Error<types::DnsRecordsForAZonePatchDnsRecordResponse>,
    > {
        let url = format!(
            "{}/zones/{}/dns_records/{}",
            self.baseurl,
            encode_path(&zone_id.to_string()),
            encode_path(&dns_record_id.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .patch(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "dns_records_for_a_zone_patch_dns_record",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///DNSSEC Details
    ///
    ///Details about DNSSEC status and configuration.
    ///
    ///Sends a `GET` request to `/zones/{zone_id}/dnssec`
    pub async fn dnssec_dnssec_details<'a>(
        &'a self,
        zone_id: &'a types::DnssecIdentifier,
    ) -> Result<
        ResponseValue<types::DnssecDnssecResponseSingle>,
        Error<types::DnssecDnssecDetailsResponse>,
    > {
        let url = format!(
            "{}/zones/{}/dnssec",
            self.baseurl,
            encode_path(&zone_id.to_string()),
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
            operation_id: "dnssec_dnssec_details",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete DNSSEC records
    ///
    ///Delete DNSSEC.
    ///
    ///Sends a `DELETE` request to `/zones/{zone_id}/dnssec`
    pub async fn dnssec_delete_dnssec_records<'a>(
        &'a self,
        zone_id: &'a types::DnssecIdentifier,
        body: &'a ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    ) -> Result<
        ResponseValue<types::DnssecDeleteDnssecResponseSingle>,
        Error<types::DnssecDeleteDnssecRecordsResponse>,
    > {
        let url = format!(
            "{}/zones/{}/dnssec",
            self.baseurl,
            encode_path(&zone_id.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .delete(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "dnssec_delete_dnssec_records",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Edit DNSSEC Status
    ///
    ///Enable or disable DNSSEC.
    ///
    ///Sends a `PATCH` request to `/zones/{zone_id}/dnssec`
    pub async fn dnssec_edit_dnssec_status<'a>(
        &'a self,
        zone_id: &'a types::DnssecIdentifier,
        body: &'a types::DnssecEditDnssecStatusBody,
    ) -> Result<
        ResponseValue<types::DnssecDnssecResponseSingle>,
        Error<types::DnssecEditDnssecStatusResponse>,
    > {
        let url = format!(
            "{}/zones/{}/dnssec",
            self.baseurl,
            encode_path(&zone_id.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .patch(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "dnssec_edit_dnssec_status",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get Automatic SSL/TLS enrollment status for the given zone
    ///
    ///If the system is enabled, the response will include next_scheduled_scan,
    /// representing the next time this zone will be scanned and the zone's
    /// ssl/tls encryption mode is potentially upgraded by the system. If the
    /// system is disabled, next_scheduled_scan will not be present in the
    /// response body.
    ///
    ///Sends a `GET` request to `/zones/{zone_id}/settings/ssl_automatic_mode`
    pub async fn ssl_detector_automatic_mode_get_enrollment<'a>(
        &'a self,
        zone_id: &'a types::CacheIdentifier,
    ) -> Result<
        ResponseValue<types::CacheApiResponseSingleId>,
        Error<types::CacheApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/zones/{}/settings/ssl_automatic_mode",
            self.baseurl,
            encode_path(&zone_id.to_string()),
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
            operation_id: "ssl_detector_automatic_mode_get_enrollment",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Patch Automatic SSL/TLS Enrollment status for given zone
    ///
    ///The automatic system is enabled when this endpoint is hit with value in
    /// the request body is set to "auto", and disabled when the request body
    /// value is set to "custom".
    ///
    ///Sends a `PATCH` request to
    /// `/zones/{zone_id}/settings/ssl_automatic_mode`
    pub async fn ssl_detector_automatic_mode_patch_enrollment<'a>(
        &'a self,
        zone_id: &'a types::CacheIdentifier,
        body: &'a types::CacheSchemasPatch,
    ) -> Result<
        ResponseValue<types::CacheApiResponseSingleId>,
        Error<types::CacheApiResponseCommonFailure>,
    > {
        let url = format!(
            "{}/zones/{}/settings/ssl_automatic_mode",
            self.baseurl,
            encode_path(&zone_id.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .patch(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "ssl_detector_automatic_mode_patch_enrollment",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::from_response(response).await?,
            )),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
}

/// Items consumers will typically use such as the Client.
pub mod prelude {
    #[allow(unused_imports)]
    pub use super::Client;
}
