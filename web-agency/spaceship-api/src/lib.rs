#[allow(unused_imports)]
use progenitor_client::{encode_path, ClientHooks, OperationInfo, RequestBuilderExt};
#[allow(unused_imports)]
pub use progenitor_client::{ByteStream, ClientInfo, Error, ResponseValue};

pub mod compat;
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

    ///Used to map a domain name to its corresponding IPv4 address
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map a domain name to its corresponding IPv4
    /// address",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "address",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "address": {
    ///      "$ref": "#/components/schemas/ipV4Address"
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "A"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AResourceRecord {
        pub address: IpV4Address,
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: AResourceRecordType,
    }

    ///Used to map a domain name to its corresponding IPv4 address
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map a domain name to its corresponding IPv4
    /// address",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "address",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "address": {
    ///      "$ref": "#/components/schemas/ipV4Address"
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "A"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AResourceRecordCreateOrUpdateItem {
        pub address: IpV4Address,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: AResourceRecordCreateOrUpdateItemType,
    }

    ///`AResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "A"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum AResourceRecordCreateOrUpdateItemType {
        A,
    }

    impl ::std::fmt::Display for AResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::A => f.write_str("A"),
            }
        }
    }

    impl ::std::str::FromStr for AResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "A" => Ok(Self::A),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for AResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to map a domain name to its corresponding IPv4 address
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map a domain name to its corresponding IPv4
    /// address",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "address",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "address": {
    ///      "$ref": "#/components/schemas/ipV4Address"
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "A"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AResourceRecordDeleteItem {
        pub address: IpV4Address,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        #[serde(rename = "type")]
        pub type_: AResourceRecordDeleteItemType,
    }

    ///`AResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "A"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum AResourceRecordDeleteItemType {
        A,
    }

    impl ::std::fmt::Display for AResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::A => f.write_str("A"),
            }
        }
    }

    impl ::std::str::FromStr for AResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "A" => Ok(Self::A),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for AResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`AResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "A"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum AResourceRecordType {
        A,
    }

    impl ::std::fmt::Display for AResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::A => f.write_str("A"),
            }
        }
    }

    impl ::std::str::FromStr for AResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "A" => Ok(Self::A),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for AResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to map a domain name to its corresponding IPv6 address
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map a domain name to its corresponding IPv6
    /// address",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "address",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "address": {
    ///      "$ref": "#/components/schemas/ipV6Address"
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "AAAA"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AaaaResourceRecord {
        pub address: IpV6Address,
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: AaaaResourceRecordType,
    }

    ///Used to map a domain name to its corresponding IPv6 address
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map a domain name to its corresponding IPv6
    /// address",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "address",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "address": {
    ///      "$ref": "#/components/schemas/ipV6Address"
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "AAAA"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AaaaResourceRecordCreateOrUpdateItem {
        pub address: IpV6Address,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: AaaaResourceRecordCreateOrUpdateItemType,
    }

    ///`AaaaResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "AAAA"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum AaaaResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "AAAA")]
        Aaaa,
    }

    impl ::std::fmt::Display for AaaaResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Aaaa => f.write_str("AAAA"),
            }
        }
    }

    impl ::std::str::FromStr for AaaaResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "AAAA" => Ok(Self::Aaaa),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for AaaaResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AaaaResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AaaaResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to map a domain name to its corresponding IPv6 address
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map a domain name to its corresponding IPv6
    /// address",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "address",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "address": {
    ///      "$ref": "#/components/schemas/ipV6Address"
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "AAAA"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AaaaResourceRecordDeleteItem {
        pub address: IpV6Address,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        #[serde(rename = "type")]
        pub type_: AaaaResourceRecordDeleteItemType,
    }

    ///`AaaaResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "AAAA"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum AaaaResourceRecordDeleteItemType {
        #[serde(rename = "AAAA")]
        Aaaa,
    }

    impl ::std::fmt::Display for AaaaResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Aaaa => f.write_str("AAAA"),
            }
        }
    }

    impl ::std::str::FromStr for AaaaResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "AAAA" => Ok(Self::Aaaa),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for AaaaResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AaaaResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AaaaResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`AaaaResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "AAAA"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum AaaaResourceRecordType {
        #[serde(rename = "AAAA")]
        Aaaa,
    }

    impl ::std::fmt::Display for AaaaResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Aaaa => f.write_str("AAAA"),
            }
        }
    }

    impl ::std::str::FromStr for AaaaResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "AAAA" => Ok(Self::Aaaa),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for AaaaResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AaaaResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AaaaResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to create a CNAME-like behavior for apex domain where CNAME is not
    /// allowed
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to create a CNAME-like behavior for apex domain
    /// where CNAME is not allowed",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "aliasName",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "aliasName": {
    ///      "description": "Canonical (true) domain name that is used to
    /// resolve resource records. Implements CNAME-like behavior for apex domain
    /// where CNAME is not allowed",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "ALIAS"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AliasResourceRecord {
        ///Canonical (true) domain name that is used to resolve resource
        /// records. Implements CNAME-like behavior for apex domain where CNAME
        /// is not allowed
        #[serde(rename = "aliasName")]
        pub alias_name: HostNameValue,
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: AliasResourceRecordType,
    }

    ///Used to create a CNAME-like behavior for apex domain where CNAME is not
    /// allowed
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to create a CNAME-like behavior for apex domain
    /// where CNAME is not allowed",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "aliasName",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "aliasName": {
    ///      "description": "Canonical (true) domain name that is used to
    /// resolve resource records. Implements CNAME-like behavior for apex domain
    /// where CNAME is not allowed",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "ALIAS"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AliasResourceRecordCreateOrUpdateItem {
        ///Canonical (true) domain name that is used to resolve resource
        /// records. Implements CNAME-like behavior for apex domain where CNAME
        /// is not allowed
        #[serde(rename = "aliasName")]
        pub alias_name: HostNameValue,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: AliasResourceRecordCreateOrUpdateItemType,
    }

    ///`AliasResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "ALIAS"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum AliasResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "ALIAS")]
        Alias,
    }

    impl ::std::fmt::Display for AliasResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Alias => f.write_str("ALIAS"),
            }
        }
    }

    impl ::std::str::FromStr for AliasResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "ALIAS" => Ok(Self::Alias),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for AliasResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AliasResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AliasResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to create a CNAME-like behavior for apex domain where CNAME is not
    /// allowed
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to create a CNAME-like behavior for apex domain
    /// where CNAME is not allowed",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "aliasName",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "aliasName": {
    ///      "description": "Canonical (true) domain name that is used to
    /// resolve resource records. Implements CNAME-like behavior for apex domain
    /// where CNAME is not allowed",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "ALIAS"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AliasResourceRecordDeleteItem {
        ///Canonical (true) domain name that is used to resolve resource
        /// records. Implements CNAME-like behavior for apex domain where CNAME
        /// is not allowed
        #[serde(rename = "aliasName")]
        pub alias_name: HostNameValue,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        #[serde(rename = "type")]
        pub type_: AliasResourceRecordDeleteItemType,
    }

    ///`AliasResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "ALIAS"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum AliasResourceRecordDeleteItemType {
        #[serde(rename = "ALIAS")]
        Alias,
    }

    impl ::std::fmt::Display for AliasResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Alias => f.write_str("ALIAS"),
            }
        }
    }

    impl ::std::str::FromStr for AliasResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "ALIAS" => Ok(Self::Alias),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for AliasResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AliasResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AliasResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`AliasResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "ALIAS"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum AliasResourceRecordType {
        #[serde(rename = "ALIAS")]
        Alias,
    }

    impl ::std::fmt::Display for AliasResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Alias => f.write_str("ALIAS"),
            }
        }
    }

    impl ::std::str::FromStr for AliasResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "ALIAS" => Ok(Self::Alias),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for AliasResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AliasResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AliasResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`AsyncOperationData`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "createdAt",
    ///    "status",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "createdAt": {
    ///      "description": "The time the async operation was created.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/isoDate"
    ///        }
    ///      ]
    ///    },
    ///    "details": {
    ///      "description": "Any detail attached to the async operation
    /// resource.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/AsyncOperationDetails"
    ///        }
    ///      ]
    ///    },
    ///    "modifiedAt": {
    ///      "description": "The time the async operation was last modified.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/isoDate"
    ///        }
    ///      ]
    ///    },
    ///    "status": {
    ///      "description": "The status of the async operation.",
    ///      "examples": [
    ///        "pending"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/AsyncOperationStatus"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "examples": [
    ///        "domains_Create"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 255,
    ///      "minLength": 1,
    ///      "pattern": "\\w+"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AsyncOperationData {
        ///The time the async operation was created.
        #[serde(rename = "createdAt")]
        pub created_at: IsoDate,
        ///Any detail attached to the async operation resource.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub details: ::std::option::Option<AsyncOperationDetails>,
        ///The time the async operation was last modified.
        #[serde(
            rename = "modifiedAt",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub modified_at: ::std::option::Option<IsoDate>,
        ///The status of the async operation.
        pub status: AsyncOperationStatus,
        #[serde(rename = "type")]
        pub type_: AsyncOperationDataType,
    }

    ///`AsyncOperationDataType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    "domains_Create"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 255,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct AsyncOperationDataType(::std::string::String);
    impl ::std::ops::Deref for AsyncOperationDataType {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<AsyncOperationDataType> for ::std::string::String {
        fn from(value: AsyncOperationDataType) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for AsyncOperationDataType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 255usize {
                return Err("longer than 255 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("\\w+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"\\w+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for AsyncOperationDataType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AsyncOperationDataType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AsyncOperationDataType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for AsyncOperationDataType {
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

    ///Async operation details
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Async operation details",
    ///  "type": "object"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct AsyncOperationDetails(
        pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    );
    impl ::std::ops::Deref for AsyncOperationDetails {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }

    impl ::std::convert::From<AsyncOperationDetails>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: AsyncOperationDetails) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for AsyncOperationDetails
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }

    ///`AsyncOperationStatus`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "pending",
    ///    "failed",
    ///    "success"
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
    pub enum AsyncOperationStatus {
        #[serde(rename = "pending")]
        Pending,
        #[serde(rename = "failed")]
        Failed,
        #[serde(rename = "success")]
        Success,
    }

    impl ::std::fmt::Display for AsyncOperationStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Pending => f.write_str("pending"),
                Self::Failed => f.write_str("failed"),
                Self::Success => f.write_str("success"),
            }
        }
    }

    impl ::std::str::FromStr for AsyncOperationStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "pending" => Ok(Self::Pending),
                "failed" => Ok(Self::Failed),
                "success" => Ok(Self::Success),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for AsyncOperationStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AsyncOperationStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AsyncOperationStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Attribute details
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Attribute details",
    ///  "type": "object",
    ///  "required": [
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "type": {
    ///      "type": "string",
    ///      "maxLength": 32,
    ///      "minLength": 2,
    ///      "pattern": "\\w+"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AttributeDetails {
        #[serde(rename = "type")]
        pub type_: AttributeDetailsType,
    }

    ///`AttributeDetailsType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 32,
    ///  "minLength": 2,
    ///  "pattern": "\\w+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct AttributeDetailsType(::std::string::String);
    impl ::std::ops::Deref for AttributeDetailsType {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<AttributeDetailsType> for ::std::string::String {
        fn from(value: AttributeDetailsType) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for AttributeDetailsType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("\\w+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"\\w+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for AttributeDetailsType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AttributeDetailsType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AttributeDetailsType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for AttributeDetailsType {
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

    ///`AttributesContactsAttributesResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "contactId"
    ///  ],
    ///  "properties": {
    ///    "contactId": {
    ///      "$ref": "#/components/schemas/contactId"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct AttributesContactsAttributesResponse {
        #[serde(rename = "contactId")]
        pub contact_id: ContactId,
    }

    ///Authorization code for the domain
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Authorization code for the domain",
    ///  "examples": [
    ///    "abc@#$123%^&def"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 50,
    ///  "minLength": 1,
    ///  "pattern": "^.+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct AuthCode(::std::string::String);
    impl ::std::ops::Deref for AuthCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<AuthCode> for ::std::string::String {
        fn from(value: AuthCode) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for AuthCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 50usize {
                return Err("longer than 50 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^.+$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^.+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for AuthCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for AuthCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for AuthCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for AuthCode {
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

    ///Used to map an alias or subdomain to its canonical (true) domain name
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map an alias or subdomain to its canonical
    /// (true) domain name",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "cname",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "cname": {
    ///      "description": "Canonical (true) domain name that is used to
    /// resolve resource records",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "CNAME"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CNameResourceRecord {
        ///Canonical (true) domain name that is used to resolve resource
        /// records
        pub cname: HostNameValue,
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: CNameResourceRecordType,
    }

    ///Used to map an alias or subdomain to its canonical (true) domain name
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map an alias or subdomain to its canonical
    /// (true) domain name",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "cname",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "cname": {
    ///      "description": "Canonical (true) domain name that is used to
    /// resolve resource records",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "CNAME"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CNameResourceRecordCreateOrUpdateItem {
        ///Canonical (true) domain name that is used to resolve resource
        /// records
        pub cname: HostNameValue,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: CNameResourceRecordCreateOrUpdateItemType,
    }

    ///`CNameResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "CNAME"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum CNameResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "CNAME")]
        Cname,
    }

    impl ::std::fmt::Display for CNameResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Cname => f.write_str("CNAME"),
            }
        }
    }

    impl ::std::str::FromStr for CNameResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "CNAME" => Ok(Self::Cname),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CNameResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CNameResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CNameResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to map an alias or subdomain to its canonical (true) domain name
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map an alias or subdomain to its canonical
    /// (true) domain name",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "cname",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "cname": {
    ///      "description": "Canonical (true) domain name that is used to
    /// resolve resource records",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "CNAME"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CNameResourceRecordDeleteItem {
        ///Canonical (true) domain name that is used to resolve resource
        /// records
        pub cname: HostNameValue,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        #[serde(rename = "type")]
        pub type_: CNameResourceRecordDeleteItemType,
    }

    ///`CNameResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "CNAME"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum CNameResourceRecordDeleteItemType {
        #[serde(rename = "CNAME")]
        Cname,
    }

    impl ::std::fmt::Display for CNameResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Cname => f.write_str("CNAME"),
            }
        }
    }

    impl ::std::str::FromStr for CNameResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "CNAME" => Ok(Self::Cname),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CNameResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CNameResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CNameResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`CNameResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "CNAME"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum CNameResourceRecordType {
        #[serde(rename = "CNAME")]
        Cname,
    }

    impl ::std::fmt::Display for CNameResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Cname => f.write_str("CNAME"),
            }
        }
    }

    impl ::std::str::FromStr for CNameResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "CNAME" => Ok(Self::Cname),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CNameResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CNameResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CNameResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`CaAttributeDetails`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/AttributeDetails"
    ///    }
    ///  ],
    ///  "required": [
    ///    "agreementValue",
    ///    "language",
    ///    "registrantCiraCategory",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "agreementValue": {
    ///      "description": "When you register a domain, you agree to follow the
    /// rules set out by the registry. This agreement applies to all
    /// registrations associated with your contact information.",
    ///      "type": "boolean"
    ///    },
    ///    "language": {
    ///      "description": "Language code in corresponding format.",
    ///      "examples": [
    ///        "EN"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "EN",
    ///        "FR"
    ///      ]
    ///    },
    ///    "registrantCiraCategory": {
    ///      "description": "Cira category for registrant contact",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/CaCiraCategory"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "ca"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CaAttributeDetails {
        ///When you register a domain, you agree to follow the rules set out by
        /// the registry. This agreement applies to all registrations associated
        /// with your contact information.
        #[serde(rename = "agreementValue")]
        pub agreement_value: bool,
        ///Language code in corresponding format.
        pub language: CaAttributeDetailsLanguage,
        ///Cira category for registrant contact
        #[serde(rename = "registrantCiraCategory")]
        pub registrant_cira_category: CaCiraCategory,
        #[serde(rename = "type")]
        pub type_: CaAttributeDetailsType,
    }

    ///Language code in corresponding format.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Language code in corresponding format.",
    ///  "examples": [
    ///    "EN"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "EN",
    ///    "FR"
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
    pub enum CaAttributeDetailsLanguage {
        #[serde(rename = "EN")]
        En,
        #[serde(rename = "FR")]
        Fr,
    }

    impl ::std::fmt::Display for CaAttributeDetailsLanguage {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::En => f.write_str("EN"),
                Self::Fr => f.write_str("FR"),
            }
        }
    }

    impl ::std::str::FromStr for CaAttributeDetailsLanguage {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "EN" => Ok(Self::En),
                "FR" => Ok(Self::Fr),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CaAttributeDetailsLanguage {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CaAttributeDetailsLanguage {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CaAttributeDetailsLanguage {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`CaAttributeDetailsType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "ca"
    ///  ],
    ///  "maxLength": 32,
    ///  "minLength": 2,
    ///  "pattern": "\\w+"
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
    pub enum CaAttributeDetailsType {
        #[serde(rename = "ca")]
        Ca,
    }

    impl ::std::fmt::Display for CaAttributeDetailsType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Ca => f.write_str("ca"),
            }
        }
    }

    impl ::std::str::FromStr for CaAttributeDetailsType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "ca" => Ok(Self::Ca),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CaAttributeDetailsType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CaAttributeDetailsType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CaAttributeDetailsType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`CaCiraCategory`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    "CCT"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "CCO",
    ///    "CCT",
    ///    "RES",
    ///    "GOV",
    ///    "EDU",
    ///    "ASS",
    ///    "HOP",
    ///    "PRT",
    ///    "TDM",
    ///    "TRD",
    ///    "PLT",
    ///    "LAM",
    ///    "TRS",
    ///    "ABO",
    ///    "INB",
    ///    "LGR",
    ///    "OMK",
    ///    "MAJ"
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
    pub enum CaCiraCategory {
        #[serde(rename = "CCO")]
        Cco,
        #[serde(rename = "CCT")]
        Cct,
        #[serde(rename = "RES")]
        Res,
        #[serde(rename = "GOV")]
        Gov,
        #[serde(rename = "EDU")]
        Edu,
        #[serde(rename = "ASS")]
        Ass,
        #[serde(rename = "HOP")]
        Hop,
        #[serde(rename = "PRT")]
        Prt,
        #[serde(rename = "TDM")]
        Tdm,
        #[serde(rename = "TRD")]
        Trd,
        #[serde(rename = "PLT")]
        Plt,
        #[serde(rename = "LAM")]
        Lam,
        #[serde(rename = "TRS")]
        Trs,
        #[serde(rename = "ABO")]
        Abo,
        #[serde(rename = "INB")]
        Inb,
        #[serde(rename = "LGR")]
        Lgr,
        #[serde(rename = "OMK")]
        Omk,
        #[serde(rename = "MAJ")]
        Maj,
    }

    impl ::std::fmt::Display for CaCiraCategory {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Cco => f.write_str("CCO"),
                Self::Cct => f.write_str("CCT"),
                Self::Res => f.write_str("RES"),
                Self::Gov => f.write_str("GOV"),
                Self::Edu => f.write_str("EDU"),
                Self::Ass => f.write_str("ASS"),
                Self::Hop => f.write_str("HOP"),
                Self::Prt => f.write_str("PRT"),
                Self::Tdm => f.write_str("TDM"),
                Self::Trd => f.write_str("TRD"),
                Self::Plt => f.write_str("PLT"),
                Self::Lam => f.write_str("LAM"),
                Self::Trs => f.write_str("TRS"),
                Self::Abo => f.write_str("ABO"),
                Self::Inb => f.write_str("INB"),
                Self::Lgr => f.write_str("LGR"),
                Self::Omk => f.write_str("OMK"),
                Self::Maj => f.write_str("MAJ"),
            }
        }
    }

    impl ::std::str::FromStr for CaCiraCategory {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "CCO" => Ok(Self::Cco),
                "CCT" => Ok(Self::Cct),
                "RES" => Ok(Self::Res),
                "GOV" => Ok(Self::Gov),
                "EDU" => Ok(Self::Edu),
                "ASS" => Ok(Self::Ass),
                "HOP" => Ok(Self::Hop),
                "PRT" => Ok(Self::Prt),
                "TDM" => Ok(Self::Tdm),
                "TRD" => Ok(Self::Trd),
                "PLT" => Ok(Self::Plt),
                "LAM" => Ok(Self::Lam),
                "TRS" => Ok(Self::Trs),
                "ABO" => Ok(Self::Abo),
                "INB" => Ok(Self::Inb),
                "LGR" => Ok(Self::Lgr),
                "OMK" => Ok(Self::Omk),
                "MAJ" => Ok(Self::Maj),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CaCiraCategory {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CaCiraCategory {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CaCiraCategory {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Allows domain owners to specify which Certificate Authorities (CAs) are
    /// authorized to issue SSL/TLS certificates for their domain
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Allows domain owners to specify which Certificate
    /// Authorities (CAs) are authorized to issue SSL/TLS certificates for their
    /// domain",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "flag",
    ///    "tag",
    ///    "type",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "flag": {
    ///      "description": "0   - no flags are set;\n128 - indicates that the
    /// “critical bit” is set, and that CAs should halt and not issue a
    /// certificate if they don’t recognize the contents of the tag field",
    ///      "type": "number",
    ///      "enum": [
    ///        0.0,
    ///        128.0
    ///      ]
    ///    },
    ///    "tag": {
    ///      "description": "Indicates specific actions or restrictions related
    /// to certificate issuance",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/CaaTag"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "CAA"
    ///      ]
    ///    },
    ///    "value": {
    ///      "description": "Contains at most one CA identifier and optional
    /// semicolon-separated parameters",
    ///      "type": "string",
    ///      "maxLength": 256,
    ///      "pattern": "^[ -~]+$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CaaResourceRecord {
        ///0   - no flags are set;
        ///128 - indicates that the “critical bit” is set, and that CAs should
        /// halt and not issue a certificate if they don’t recognize the
        /// contents of the tag field
        pub flag: CaaResourceRecordFlag,
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Indicates specific actions or restrictions related to certificate
        /// issuance
        pub tag: CaaTag,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: CaaResourceRecordType,
        ///Contains at most one CA identifier and optional semicolon-separated
        /// parameters
        pub value: CaaResourceRecordValue,
    }

    ///Allows domain owners to specify which Certificate Authorities (CAs) are
    /// authorized to issue SSL/TLS certificates for their domain
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Allows domain owners to specify which Certificate
    /// Authorities (CAs) are authorized to issue SSL/TLS certificates for their
    /// domain",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "flag",
    ///    "tag",
    ///    "type",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "flag": {
    ///      "description": "0   - no flags are set;\n128 - indicates that the
    /// “critical bit” is set, and that CAs should halt and not issue a
    /// certificate if they don’t recognize the contents of the tag field",
    ///      "type": "number",
    ///      "enum": [
    ///        0.0,
    ///        128.0
    ///      ]
    ///    },
    ///    "tag": {
    ///      "description": "Indicates specific actions or restrictions related
    /// to certificate issuance",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/CaaTag"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "CAA"
    ///      ]
    ///    },
    ///    "value": {
    ///      "description": "Contains at most one CA identifier and optional
    /// semicolon-separated parameters",
    ///      "type": "string",
    ///      "maxLength": 256,
    ///      "pattern": "^[ -~]+$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CaaResourceRecordCreateOrUpdateItem {
        ///0   - no flags are set;
        ///128 - indicates that the “critical bit” is set, and that CAs should
        /// halt and not issue a certificate if they don’t recognize the
        /// contents of the tag field
        pub flag: CaaResourceRecordCreateOrUpdateItemFlag,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Indicates specific actions or restrictions related to certificate
        /// issuance
        pub tag: CaaTag,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: CaaResourceRecordCreateOrUpdateItemType,
        ///Contains at most one CA identifier and optional semicolon-separated
        /// parameters
        pub value: CaaResourceRecordCreateOrUpdateItemValue,
    }

    ///0   - no flags are set;
    ///128 - indicates that the “critical bit” is set, and that CAs should halt
    /// and not issue a certificate if they don’t recognize the contents of the
    /// tag field
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "0   - no flags are set;\n128 - indicates that the
    /// “critical bit” is set, and that CAs should halt and not issue a
    /// certificate if they don’t recognize the contents of the tag field",
    ///  "type": "number",
    ///  "enum": [
    ///    0.0,
    ///    128.0
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct CaaResourceRecordCreateOrUpdateItemFlag(f64);
    impl ::std::ops::Deref for CaaResourceRecordCreateOrUpdateItemFlag {
        type Target = f64;
        fn deref(&self) -> &f64 {
            &self.0
        }
    }

    impl ::std::convert::From<CaaResourceRecordCreateOrUpdateItemFlag> for f64 {
        fn from(value: CaaResourceRecordCreateOrUpdateItemFlag) -> Self {
            value.0
        }
    }

    impl ::std::convert::TryFrom<f64> for CaaResourceRecordCreateOrUpdateItemFlag {
        type Error = self::error::ConversionError;
        fn try_from(value: f64) -> ::std::result::Result<Self, self::error::ConversionError> {
            if ![0.0_f64, 128.0_f64].contains(&value) {
                Err("invalid value".into())
            } else {
                Ok(Self(value))
            }
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CaaResourceRecordCreateOrUpdateItemFlag {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            Self::try_from(<f64>::deserialize(deserializer)?)
                .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
        }
    }

    ///`CaaResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "CAA"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum CaaResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "CAA")]
        Caa,
    }

    impl ::std::fmt::Display for CaaResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Caa => f.write_str("CAA"),
            }
        }
    }

    impl ::std::str::FromStr for CaaResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "CAA" => Ok(Self::Caa),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CaaResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CaaResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CaaResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Contains at most one CA identifier and optional semicolon-separated
    /// parameters
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contains at most one CA identifier and optional
    /// semicolon-separated parameters",
    ///  "type": "string",
    ///  "maxLength": 256,
    ///  "pattern": "^[ -~]+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CaaResourceRecordCreateOrUpdateItemValue(::std::string::String);
    impl ::std::ops::Deref for CaaResourceRecordCreateOrUpdateItemValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CaaResourceRecordCreateOrUpdateItemValue> for ::std::string::String {
        fn from(value: CaaResourceRecordCreateOrUpdateItemValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CaaResourceRecordCreateOrUpdateItemValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 256usize {
                return Err("longer than 256 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[ -~]+$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[ -~]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CaaResourceRecordCreateOrUpdateItemValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CaaResourceRecordCreateOrUpdateItemValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CaaResourceRecordCreateOrUpdateItemValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CaaResourceRecordCreateOrUpdateItemValue {
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

    ///Allows domain owners to specify which Certificate Authorities (CAs) are
    /// authorized to issue SSL/TLS certificates for their domain
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Allows domain owners to specify which Certificate
    /// Authorities (CAs) are authorized to issue SSL/TLS certificates for their
    /// domain",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "flag",
    ///    "tag",
    ///    "type",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "flag": {
    ///      "description": "0   - no flags are set;\n128 - indicates that the
    /// “critical bit” is set, and that CAs should halt and not issue a
    /// certificate if they don’t recognize the contents of the tag field",
    ///      "type": "number",
    ///      "enum": [
    ///        0.0,
    ///        128.0
    ///      ]
    ///    },
    ///    "tag": {
    ///      "description": "Indicates specific actions or restrictions related
    /// to certificate issuance",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/CaaTag"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "CAA"
    ///      ]
    ///    },
    ///    "value": {
    ///      "description": "Contains at most one CA identifier and optional
    /// semicolon-separated parameters",
    ///      "type": "string",
    ///      "maxLength": 256,
    ///      "pattern": "^[ -~]+$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CaaResourceRecordDeleteItem {
        ///0   - no flags are set;
        ///128 - indicates that the “critical bit” is set, and that CAs should
        /// halt and not issue a certificate if they don’t recognize the
        /// contents of the tag field
        pub flag: CaaResourceRecordDeleteItemFlag,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Indicates specific actions or restrictions related to certificate
        /// issuance
        pub tag: CaaTag,
        #[serde(rename = "type")]
        pub type_: CaaResourceRecordDeleteItemType,
        ///Contains at most one CA identifier and optional semicolon-separated
        /// parameters
        pub value: CaaResourceRecordDeleteItemValue,
    }

    ///0   - no flags are set;
    ///128 - indicates that the “critical bit” is set, and that CAs should halt
    /// and not issue a certificate if they don’t recognize the contents of the
    /// tag field
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "0   - no flags are set;\n128 - indicates that the
    /// “critical bit” is set, and that CAs should halt and not issue a
    /// certificate if they don’t recognize the contents of the tag field",
    ///  "type": "number",
    ///  "enum": [
    ///    0.0,
    ///    128.0
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct CaaResourceRecordDeleteItemFlag(f64);
    impl ::std::ops::Deref for CaaResourceRecordDeleteItemFlag {
        type Target = f64;
        fn deref(&self) -> &f64 {
            &self.0
        }
    }

    impl ::std::convert::From<CaaResourceRecordDeleteItemFlag> for f64 {
        fn from(value: CaaResourceRecordDeleteItemFlag) -> Self {
            value.0
        }
    }

    impl ::std::convert::TryFrom<f64> for CaaResourceRecordDeleteItemFlag {
        type Error = self::error::ConversionError;
        fn try_from(value: f64) -> ::std::result::Result<Self, self::error::ConversionError> {
            if ![0.0_f64, 128.0_f64].contains(&value) {
                Err("invalid value".into())
            } else {
                Ok(Self(value))
            }
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CaaResourceRecordDeleteItemFlag {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            Self::try_from(<f64>::deserialize(deserializer)?)
                .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
        }
    }

    ///`CaaResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "CAA"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum CaaResourceRecordDeleteItemType {
        #[serde(rename = "CAA")]
        Caa,
    }

    impl ::std::fmt::Display for CaaResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Caa => f.write_str("CAA"),
            }
        }
    }

    impl ::std::str::FromStr for CaaResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "CAA" => Ok(Self::Caa),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CaaResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CaaResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CaaResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Contains at most one CA identifier and optional semicolon-separated
    /// parameters
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contains at most one CA identifier and optional
    /// semicolon-separated parameters",
    ///  "type": "string",
    ///  "maxLength": 256,
    ///  "pattern": "^[ -~]+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CaaResourceRecordDeleteItemValue(::std::string::String);
    impl ::std::ops::Deref for CaaResourceRecordDeleteItemValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CaaResourceRecordDeleteItemValue> for ::std::string::String {
        fn from(value: CaaResourceRecordDeleteItemValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CaaResourceRecordDeleteItemValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 256usize {
                return Err("longer than 256 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[ -~]+$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[ -~]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CaaResourceRecordDeleteItemValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CaaResourceRecordDeleteItemValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CaaResourceRecordDeleteItemValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CaaResourceRecordDeleteItemValue {
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

    ///0   - no flags are set;
    ///128 - indicates that the “critical bit” is set, and that CAs should halt
    /// and not issue a certificate if they don’t recognize the contents of the
    /// tag field
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "0   - no flags are set;\n128 - indicates that the
    /// “critical bit” is set, and that CAs should halt and not issue a
    /// certificate if they don’t recognize the contents of the tag field",
    ///  "type": "number",
    ///  "enum": [
    ///    0.0,
    ///    128.0
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct CaaResourceRecordFlag(f64);
    impl ::std::ops::Deref for CaaResourceRecordFlag {
        type Target = f64;
        fn deref(&self) -> &f64 {
            &self.0
        }
    }

    impl ::std::convert::From<CaaResourceRecordFlag> for f64 {
        fn from(value: CaaResourceRecordFlag) -> Self {
            value.0
        }
    }

    impl ::std::convert::TryFrom<f64> for CaaResourceRecordFlag {
        type Error = self::error::ConversionError;
        fn try_from(value: f64) -> ::std::result::Result<Self, self::error::ConversionError> {
            if ![0.0_f64, 128.0_f64].contains(&value) {
                Err("invalid value".into())
            } else {
                Ok(Self(value))
            }
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CaaResourceRecordFlag {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            Self::try_from(<f64>::deserialize(deserializer)?)
                .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
        }
    }

    ///`CaaResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "CAA"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum CaaResourceRecordType {
        #[serde(rename = "CAA")]
        Caa,
    }

    impl ::std::fmt::Display for CaaResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Caa => f.write_str("CAA"),
            }
        }
    }

    impl ::std::str::FromStr for CaaResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "CAA" => Ok(Self::Caa),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CaaResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CaaResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CaaResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Contains at most one CA identifier and optional semicolon-separated
    /// parameters
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contains at most one CA identifier and optional
    /// semicolon-separated parameters",
    ///  "type": "string",
    ///  "maxLength": 256,
    ///  "pattern": "^[ -~]+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CaaResourceRecordValue(::std::string::String);
    impl ::std::ops::Deref for CaaResourceRecordValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CaaResourceRecordValue> for ::std::string::String {
        fn from(value: CaaResourceRecordValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CaaResourceRecordValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 256usize {
                return Err("longer than 256 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[ -~]+$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[ -~]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CaaResourceRecordValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CaaResourceRecordValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CaaResourceRecordValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CaaResourceRecordValue {
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

    ///`CaaTag`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "issue",
    ///    "issuewild",
    ///    "iodef"
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
    pub enum CaaTag {
        #[serde(rename = "issue")]
        Issue,
        #[serde(rename = "issuewild")]
        Issuewild,
        #[serde(rename = "iodef")]
        Iodef,
    }

    impl ::std::fmt::Display for CaaTag {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Issue => f.write_str("issue"),
                Self::Issuewild => f.write_str("issuewild"),
                Self::Iodef => f.write_str("iodef"),
            }
        }
    }

    impl ::std::str::FromStr for CaaTag {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "issue" => Ok(Self::Issue),
                "issuewild" => Ok(Self::Issuewild),
                "iodef" => Ok(Self::Iodef),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CaaTag {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CaaTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CaaTag {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Specifies the type of checkout link to be created. Each type determines
    /// a different purchase flow that buyers will experience when accessing the
    /// link.
    ///
    ///Available types:
    /// - **buyNow**: Direct purchase at a fixed price without negotiation
    /// - **leaseToOwn**: Installment-based purchase plan (coming soon)
    /// - **makeOffer**: Negotiation-based purchase where buyers submit offers
    ///   (coming soon)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the type of checkout link to be created. Each type determines a different purchase flow that buyers will experience when accessing the link.\n\nAvailable types:\n- **buyNow**: Direct purchase at a fixed price without negotiation\n- **leaseToOwn**: Installment-based purchase plan (coming soon)\n- **makeOffer**: Negotiation-based purchase where buyers submit offers (coming soon)",
    ///  "type": "string",
    ///  "enum": [
    ///    "buyNow"
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
    pub enum CheckoutLinkType {
        #[serde(rename = "buyNow")]
        BuyNow,
    }

    impl ::std::fmt::Display for CheckoutLinkType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::BuyNow => f.write_str("buyNow"),
            }
        }
    }

    impl ::std::str::FromStr for CheckoutLinkType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "buyNow" => Ok(Self::BuyNow),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for CheckoutLinkType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CheckoutLinkType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CheckoutLinkType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Contact details
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contact details",
    ///  "type": "object",
    ///  "required": [
    ///    "address1",
    ///    "city",
    ///    "country",
    ///    "email",
    ///    "firstName",
    ///    "lastName",
    ///    "phone"
    ///  ],
    ///  "properties": {
    ///    "address1": {
    ///      "description": "Address (line 1)",
    ///      "examples": [
    ///        "286 King St."
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 255,
    ///      "pattern": "^[\\d#&'()./:;A-Za-z\\s-,\\\\]+$"
    ///    },
    ///    "address2": {
    ///      "description": "Address (line 2)",
    ///      "type": "string",
    ///      "maxLength": 255,
    ///      "pattern": "^[\\d#&'()./:;A-Za-z\\s-,\\\\]+$"
    ///    },
    ///    "city": {
    ///      "description": "City",
    ///      "examples": [
    ///        "San Francisco"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 255,
    ///      "pattern":
    /// "^['.A-Za-z-][\\s'.A-Za-z-]+['.A-Za-z-]$|^['.A-Za-z-]{0,2}$"
    ///    },
    ///    "country": {
    ///      "$ref": "#/components/schemas/countryCode"
    ///    },
    ///    "email": {
    ///      "description": "Email",
    ///      "examples": [
    ///        "admin@example.com"
    ///      ],
    ///      "type": "string",
    ///      "format": "email",
    ///      "maxLength": 255,
    ///      "minLength": 3
    ///    },
    ///    "fax": {
    ///      "description": "Fax number",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/phone"
    ///        }
    ///      ]
    ///    },
    ///    "faxExt": {
    ///      "description": "Fax number extension",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/phoneExt"
    ///        }
    ///      ]
    ///    },
    ///    "firstName": {
    ///      "description": "Contact's first name",
    ///      "examples": [
    ///        "John"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 125,
    ///      "minLength": 1,
    ///      "pattern":
    /// "^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$"
    ///    },
    ///    "lastName": {
    ///      "description": "Contact's last name",
    ///      "examples": [
    ///        "Doe"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 125,
    ///      "minLength": 1,
    ///      "pattern":
    /// "^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$"
    ///    },
    ///    "organization": {
    ///      "description": "Organization/Company name",
    ///      "examples": [
    ///        "My Company"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 255,
    ///      "pattern":
    /// "^(?!\\s)[\\u0000-\\u007F]{2,}(?!\\s)[\\u0000-\\u007F]$|^((?!\\s)[\\
    /// u0000-\\u007F])*$"
    ///    },
    ///    "phone": {
    ///      "$ref": "#/components/schemas/phone"
    ///    },
    ///    "phoneExt": {
    ///      "description": "Phone number extension",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/phoneExt"
    ///        }
    ///      ]
    ///    },
    ///    "postalCode": {
    ///      "description": "Postal code",
    ///      "examples": [
    ///        "94107"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 16,
    ///      "pattern":
    /// "^[\\dA-Za-z-][\\d\\sA-Za-z-]+[\\dA-Za-z-]$|^[\\dA-Za-z-]{0,2}$"
    ///    },
    ///    "stateProvince": {
    ///      "description": "State province name",
    ///      "examples": [
    ///        "CA"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 255,
    ///      "pattern":
    /// "^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$"
    ///    },
    ///    "taxNumber": {
    ///      "description": "Tax number",
    ///      "examples": [
    ///        "123456789"
    ///      ],
    ///      "type": "string",
    ///      "format": "taxNumber",
    ///      "maxLength": 255,
    ///      "pattern":
    /// "^[\\d./A-Za-z-][\\d\\s./A-Za-z-]+[\\d./A-Za-z-]$|^[\\d./A-Za-z-]{0,2}$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ContactDetails {
        ///Address (line 1)
        pub address1: ContactDetailsAddress1,
        ///Address (line 2)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub address2: ::std::option::Option<ContactDetailsAddress2>,
        ///City
        pub city: ContactDetailsCity,
        pub country: CountryCode,
        ///Email
        pub email: ::std::string::String,
        ///Fax number
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub fax: ::std::option::Option<Phone>,
        ///Fax number extension
        #[serde(
            rename = "faxExt",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub fax_ext: ::std::option::Option<PhoneExt>,
        ///Contact's first name
        #[serde(rename = "firstName")]
        pub first_name: ContactDetailsFirstName,
        ///Contact's last name
        #[serde(rename = "lastName")]
        pub last_name: ContactDetailsLastName,
        ///Organization/Company name
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub organization: ::std::option::Option<ContactDetailsOrganization>,
        pub phone: Phone,
        ///Phone number extension
        #[serde(
            rename = "phoneExt",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub phone_ext: ::std::option::Option<PhoneExt>,
        ///Postal code
        #[serde(
            rename = "postalCode",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub postal_code: ::std::option::Option<ContactDetailsPostalCode>,
        ///State province name
        #[serde(
            rename = "stateProvince",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub state_province: ::std::option::Option<ContactDetailsStateProvince>,
        ///Tax number
        #[serde(
            rename = "taxNumber",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub tax_number: ::std::option::Option<::std::string::String>,
    }

    ///Address (line 1)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Address (line 1)",
    ///  "examples": [
    ///    "286 King St."
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 255,
    ///  "pattern": "^[\\d#&'()./:;A-Za-z\\s-,\\\\]+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ContactDetailsAddress1(::std::string::String);
    impl ::std::ops::Deref for ContactDetailsAddress1 {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ContactDetailsAddress1> for ::std::string::String {
        fn from(value: ContactDetailsAddress1) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ContactDetailsAddress1 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 255usize {
                return Err("longer than 255 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^[\\d#&'()./:;A-Za-z\\s-,\\\\]+$").unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\d#&'()./:;A-Za-z\\s-,\\\\]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ContactDetailsAddress1 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ContactDetailsAddress1 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ContactDetailsAddress1 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ContactDetailsAddress1 {
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

    ///Address (line 2)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Address (line 2)",
    ///  "type": "string",
    ///  "maxLength": 255,
    ///  "pattern": "^[\\d#&'()./:;A-Za-z\\s-,\\\\]+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ContactDetailsAddress2(::std::string::String);
    impl ::std::ops::Deref for ContactDetailsAddress2 {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ContactDetailsAddress2> for ::std::string::String {
        fn from(value: ContactDetailsAddress2) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ContactDetailsAddress2 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 255usize {
                return Err("longer than 255 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^[\\d#&'()./:;A-Za-z\\s-,\\\\]+$").unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\d#&'()./:;A-Za-z\\s-,\\\\]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ContactDetailsAddress2 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ContactDetailsAddress2 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ContactDetailsAddress2 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ContactDetailsAddress2 {
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

    ///City
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "City",
    ///  "examples": [
    ///    "San Francisco"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 255,
    ///  "pattern": "^['.A-Za-z-][\\s'.A-Za-z-]+['.A-Za-z-]$|^['.A-Za-z-]{0,2}$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ContactDetailsCity(::std::string::String);
    impl ::std::ops::Deref for ContactDetailsCity {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ContactDetailsCity> for ::std::string::String {
        fn from(value: ContactDetailsCity) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ContactDetailsCity {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 255usize {
                return Err("longer than 255 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new(
                        "^['.A-Za-z-][\\s'.A-Za-z-]+['.A-Za-z-]$|^['.A-Za-z-]{0,2}$",
                    )
                    .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^['.A-Za-z-][\\s'.A-Za-z-]+['.A-Za-z-]$|^['.A-Za-z-]{0,2}$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ContactDetailsCity {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ContactDetailsCity {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ContactDetailsCity {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ContactDetailsCity {
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

    ///Contact's first name
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contact's first name",
    ///  "examples": [
    ///    "John"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 125,
    ///  "minLength": 1,
    ///  "pattern": "^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ContactDetailsFirstName(::std::string::String);
    impl ::std::ops::Deref for ContactDetailsFirstName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ContactDetailsFirstName> for ::std::string::String {
        fn from(value: ContactDetailsFirstName) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ContactDetailsFirstName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 125usize {
                return Err("longer than 125 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$")
                        .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ContactDetailsFirstName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ContactDetailsFirstName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ContactDetailsFirstName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ContactDetailsFirstName {
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

    ///Contact's last name
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contact's last name",
    ///  "examples": [
    ///    "Doe"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 125,
    ///  "minLength": 1,
    ///  "pattern": "^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ContactDetailsLastName(::std::string::String);
    impl ::std::ops::Deref for ContactDetailsLastName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ContactDetailsLastName> for ::std::string::String {
        fn from(value: ContactDetailsLastName) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ContactDetailsLastName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 125usize {
                return Err("longer than 125 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$")
                        .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ContactDetailsLastName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ContactDetailsLastName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ContactDetailsLastName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ContactDetailsLastName {
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

    ///Organization/Company name
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Organization/Company name",
    ///  "examples": [
    ///    "My Company"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 255,
    ///  "pattern":
    /// "^(?!\\s)[\\u0000-\\u007F]{2,}(?!\\s)[\\u0000-\\u007F]$|^((?!\\s)[\\
    /// u0000-\\u007F])*$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ContactDetailsOrganization(::std::string::String);
    impl ::std::ops::Deref for ContactDetailsOrganization {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ContactDetailsOrganization> for ::std::string::String {
        fn from(value: ContactDetailsOrganization) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ContactDetailsOrganization {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 255usize {
                return Err("longer than 255 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> = ::std::sync::LazyLock::new(
                || {
                    :: regress :: Regex :: new ("^(?!\\s)[\\u0000-\\u007F]{2,}(?!\\s)[\\u0000-\\u007F]$|^((?!\\s)[\\u0000-\\u007F])*$") . unwrap ()
                },
            );
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^(?!\\s)[\\u0000-\\u007F]{2,}(?!\\s)[\\u0000-\\u007F]$|^((?!\\s)[\\u0000-\\u007F])*$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ContactDetailsOrganization {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ContactDetailsOrganization {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ContactDetailsOrganization {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ContactDetailsOrganization {
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

    ///Postal code
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Postal code",
    ///  "examples": [
    ///    "94107"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 16,
    ///  "pattern":
    /// "^[\\dA-Za-z-][\\d\\sA-Za-z-]+[\\dA-Za-z-]$|^[\\dA-Za-z-]{0,2}$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ContactDetailsPostalCode(::std::string::String);
    impl ::std::ops::Deref for ContactDetailsPostalCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ContactDetailsPostalCode> for ::std::string::String {
        fn from(value: ContactDetailsPostalCode) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ContactDetailsPostalCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 16usize {
                return Err("longer than 16 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new(
                        "^[\\dA-Za-z-][\\d\\sA-Za-z-]+[\\dA-Za-z-]$|^[\\dA-Za-z-]{0,2}$",
                    )
                    .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^[\\dA-Za-z-][\\d\\sA-Za-z-]+[\\dA-Za-z-]$|^[\\dA-Za-z-]{0,2}$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ContactDetailsPostalCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ContactDetailsPostalCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ContactDetailsPostalCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ContactDetailsPostalCode {
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

    ///State province name
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "State province name",
    ///  "examples": [
    ///    "CA"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 255,
    ///  "pattern": "^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ContactDetailsStateProvince(::std::string::String);
    impl ::std::ops::Deref for ContactDetailsStateProvince {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ContactDetailsStateProvince> for ::std::string::String {
        fn from(value: ContactDetailsStateProvince) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ContactDetailsStateProvince {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 255usize {
                return Err("longer than 255 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$")
                        .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^['A-Za-z-][\\s'A-Z`a-z-]+['A-Za-z-]$|^['A-Za-z-]{0,2}$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ContactDetailsStateProvince {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ContactDetailsStateProvince {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ContactDetailsStateProvince {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ContactDetailsStateProvince {
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

    ///Contact ID
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contact ID",
    ///  "examples": [
    ///    "1ZdMXpapqp9sle5dl8BlppTJXAzf5"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32,
    ///  "minLength": 27,
    ///  "pattern": "[a-zA-Z0-9]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ContactId(::std::string::String);
    impl ::std::ops::Deref for ContactId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ContactId> for ::std::string::String {
        fn from(value: ContactId) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ContactId {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            if value.chars().count() < 27usize {
                return Err("shorter than 27 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("[a-zA-Z0-9]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"[a-zA-Z0-9]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ContactId {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ContactId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ContactId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ContactId {
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

    ///`ContactsSaveResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "contactId"
    ///  ],
    ///  "properties": {
    ///    "contactId": {
    ///      "description": "Response with contactId generated, if contact was
    /// created, or existing contact's one.",
    ///      "examples": [
    ///        "1ZdMXpapqp9sle5dl8BlppTJXAzf3"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 32,
    ///      "minLength": 27,
    ///      "pattern": "[a-zA-Z0-9]+"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ContactsSaveResponse {
        ///Response with contactId generated, if contact was created, or
        /// existing contact's one.
        #[serde(rename = "contactId")]
        pub contact_id: ContactsSaveResponseContactId,
    }

    ///Response with contactId generated, if contact was created, or existing
    /// contact's one.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Response with contactId generated, if contact was
    /// created, or existing contact's one.",
    ///  "examples": [
    ///    "1ZdMXpapqp9sle5dl8BlppTJXAzf3"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32,
    ///  "minLength": 27,
    ///  "pattern": "[a-zA-Z0-9]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ContactsSaveResponseContactId(::std::string::String);
    impl ::std::ops::Deref for ContactsSaveResponseContactId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ContactsSaveResponseContactId> for ::std::string::String {
        fn from(value: ContactsSaveResponseContactId) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ContactsSaveResponseContactId {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            if value.chars().count() < 27usize {
                return Err("shorter than 27 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("[a-zA-Z0-9]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"[a-zA-Z0-9]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ContactsSaveResponseContactId {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ContactsSaveResponseContactId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ContactsSaveResponseContactId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ContactsSaveResponseContactId {
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

    ///Country code (ISO 3166-1 alpha-2)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Country code (ISO 3166-1 alpha-2)",
    ///  "examples": [
    ///    "US"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 2,
    ///  "minLength": 2,
    ///  "pattern": "^[A-Z]{2}$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct CountryCode(::std::string::String);
    impl ::std::ops::Deref for CountryCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<CountryCode> for ::std::string::String {
        fn from(value: CountryCode) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for CountryCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 2usize {
                return Err("longer than 2 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[A-Z]{2}$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[A-Z]{2}$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for CountryCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for CountryCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for CountryCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for CountryCode {
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

    ///Request payload for creating a checkout link. A checkout link allows
    /// potential buyers to purchase a domain listed in SellerHub through a
    /// direct URL.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Request payload for creating a checkout link. A
    /// checkout link allows potential buyers to purchase a domain listed in
    /// SellerHub through a direct URL.",
    ///  "type": "object",
    ///  "required": [
    ///    "domainName",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "basePrice": {
    ///      "description": "The price at which the domain will be offered in the checkout link. This is the amount the buyer will pay to purchase the domain. If not provided, the domain's Buy It Now price will be used.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/Price"
    ///        }
    ///      ]
    ///    },
    ///    "domainName": {
    ///      "description": "The domain name to create a checkout link for. Must
    /// be a domain currently listed in SellerHub.",
    ///      "examples": [
    ///        "example.com"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/sellerhubDomainName"
    ///        }
    ///      ]
    ///    },
    ///    "feePercentageShare": {
    ///      "description": "Commission split configuration. Determines how the
    /// platform commission is divided between seller and buyer. If omitted,
    /// defaults to 100% seller / 0% buyer.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/FeePercentageShare"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "description": "Specifies the checkout flow type. Currently
    /// supports 'BuyNow' for immediate purchase at the specified price.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/CheckoutLinkType"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateCheckoutLinkRequest {
        ///The price at which the domain will be offered in the checkout link.
        /// This is the amount the buyer will pay to purchase the domain. If not
        /// provided, the domain's Buy It Now price will be used.
        #[serde(
            rename = "basePrice",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub base_price: ::std::option::Option<Price>,
        ///The domain name to create a checkout link for. Must be a domain
        /// currently listed in SellerHub.
        #[serde(rename = "domainName")]
        pub domain_name: SellerhubDomainName,
        ///Commission split configuration. Determines how the platform
        /// commission is divided between seller and buyer. If omitted, defaults
        /// to 100% seller / 0% buyer.
        #[serde(
            rename = "feePercentageShare",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub fee_percentage_share: ::std::option::Option<FeePercentageShare>,
        ///Specifies the checkout flow type. Currently supports 'BuyNow' for
        /// immediate purchase at the specified price.
        #[serde(rename = "type")]
        pub type_: CheckoutLinkType,
    }

    ///Response containing the newly created checkout link details. The
    /// checkout link can be shared with potential buyers for direct domain
    /// purchase.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Response containing the newly created checkout link
    /// details. The checkout link can be shared with potential buyers for
    /// direct domain purchase.",
    ///  "type": "object",
    ///  "required": [
    ///    "url"
    ///  ],
    ///  "properties": {
    ///    "url": {
    ///      "description": "The full URL of the checkout link. Share this URL with potential buyers to allow them to purchase the domain directly.",
    ///      "examples": [
    ///        "https://www.spaceship.com/s/buy/adsept.com/EhA4rDdGc9lTP320"
    ///      ],
    ///      "type": "string",
    ///      "format": "uri",
    ///      "maxLength": 2048
    ///    },
    ///    "validTill": {
    ///      "description": "The expiration date and time of the checkout link
    /// in ISO-8601 format. After this time, the link will no longer be valid
    /// for purchases. If not provided, the link does not have an expiration
    /// date.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/isoDate"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateCheckoutLinkResponse {
        ///The full URL of the checkout link. Share this URL with potential
        /// buyers to allow them to purchase the domain directly.
        pub url: ::std::string::String,
        ///The expiration date and time of the checkout link in ISO-8601
        /// format. After this time, the link will no longer be valid for
        /// purchases. If not provided, the link does not have an expiration
        /// date.
        #[serde(
            rename = "validTill",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub valid_till: ::std::option::Option<IsoDate>,
    }

    ///Create SellerHub domain request
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Create SellerHub domain request",
    ///  "type": "object",
    ///  "required": [
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "binPrice": {
    ///      "description": "Buy It Now (BIN) price for the domain",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/Price"
    ///        }
    ///      ]
    ///    },
    ///    "binPriceEnabled": {
    ///      "description": "Enable or disable the Buy It Now (BIN) option",
    ///      "type": "boolean"
    ///    },
    ///    "description": {
    ///      "description": "Domain description",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/domainDescription"
    ///        }
    ///      ]
    ///    },
    ///    "displayName": {
    ///      "description": "Display name for the domain",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/displayName"
    ///        }
    ///      ]
    ///    },
    ///    "minPrice": {
    ///      "description": "Minimum offer price for the domain",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/Price"
    ///        }
    ///      ]
    ///    },
    ///    "minPriceEnabled": {
    ///      "description": "Enable or disable offer negotiation with minimum
    /// price",
    ///      "type": "boolean"
    ///    },
    ///    "name": {
    ///      "description": "Domain name in Unicode format",
    ///      "examples": [
    ///        "spaceship.com"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/sellerhubDomainName"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct CreateSellerHubDomainRequest {
        ///Buy It Now (BIN) price for the domain
        #[serde(
            rename = "binPrice",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub bin_price: ::std::option::Option<Price>,
        ///Enable or disable the Buy It Now (BIN) option
        #[serde(
            rename = "binPriceEnabled",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub bin_price_enabled: ::std::option::Option<bool>,
        ///Domain description
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub description: ::std::option::Option<DomainDescription>,
        ///Display name for the domain
        #[serde(
            rename = "displayName",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub display_name: ::std::option::Option<DisplayName>,
        ///Minimum offer price for the domain
        #[serde(
            rename = "minPrice",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub min_price: ::std::option::Option<Price>,
        ///Enable or disable offer negotiation with minimum price
        #[serde(
            rename = "minPriceEnabled",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub min_price_enabled: ::std::option::Option<bool>,
        ///Domain name in Unicode format
        pub name: SellerhubDomainName,
    }

    ///Currency code following ISO 4217 standard. Currently only USD is
    /// supported.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Currency code following ISO 4217 standard. Currently
    /// only USD is supported.",
    ///  "examples": [
    ///    "USD"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 3,
    ///  "minLength": 3,
    ///  "pattern": "^USD$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct Currency(::std::string::String);
    impl ::std::ops::Deref for Currency {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<Currency> for ::std::string::String {
        fn from(value: Currency) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for Currency {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 3usize {
                return Err("longer than 3 characters".into());
            }
            if value.chars().count() < 3usize {
                return Err("shorter than 3 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^USD$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^USD$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for Currency {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for Currency {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for Currency {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for Currency {
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

    ///Display name for a domain with original capitalization
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Display name for a domain with original
    /// capitalization",
    ///  "examples": [
    ///    "SpaceShip.com"
    ///  ],
    ///  "type": "string",
    ///  "format": "domain",
    ///  "maxLength": 255,
    ///  "minLength": 4
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
    pub struct DisplayName(pub ::std::string::String);
    impl ::std::ops::Deref for DisplayName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DisplayName> for ::std::string::String {
        fn from(value: DisplayName) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::string::String> for DisplayName {
        fn from(value: ::std::string::String) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DisplayName {
        type Err = ::std::convert::Infallible;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::fmt::Display for DisplayName {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Domain availability check result
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Domain availability check result",
    ///  "type": "object",
    ///  "required": [
    ///    "domain",
    ///    "premiumPricing",
    ///    "result"
    ///  ],
    ///  "properties": {
    ///    "domain": {
    ///      "readOnly": true,
    ///      "examples": [
    ///        "spaceship.dev"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/domainName"
    ///        }
    ///      ]
    ///    },
    ///    "premiumPricing": {
    ///      "readOnly": true,
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/DomainPriceDetails"
    ///      }
    ///    },
    ///    "result": {
    ///      "readOnly": true,
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/DomainAvailabilityStatus"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainAvailabilityResult {
        pub domain: DomainName,
        #[serde(rename = "premiumPricing")]
        pub premium_pricing: ::std::vec::Vec<DomainPriceDetails>,
        pub result: DomainAvailabilityStatus,
    }

    ///Domain availability status. Possible values include:
    /// * `available` - The domain is available for registration.
    /// * `taken` - Domain is already taken and available for transfer.
    /// * `invalidDomainName` - Domain name is invalid.
    /// * `tldNotSupported` - Specified TLD is not supported.
    /// * `unexpectedError` - An error occurred while checking domain
    ///   availability. The system was unable to determine the domain's status
    ///   due to a technical issue.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Domain availability status. Possible values include:\n*
    /// `available` - The domain is available for registration.\n* `taken` -
    /// Domain is already taken and available for transfer.\n*
    /// `invalidDomainName` - Domain name is invalid.\n* `tldNotSupported` -
    /// Specified TLD is not supported.\n* `unexpectedError` - An error occurred
    /// while checking domain availability. The system was unable to determine
    /// the domain's status due to a technical issue.",
    ///  "examples": [
    ///    "available"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "available",
    ///    "taken",
    ///    "invalidDomainName",
    ///    "tldNotSupported",
    ///    "unexpectedError"
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
    pub enum DomainAvailabilityStatus {
        #[serde(rename = "available")]
        Available,
        #[serde(rename = "taken")]
        Taken,
        #[serde(rename = "invalidDomainName")]
        InvalidDomainName,
        #[serde(rename = "tldNotSupported")]
        TldNotSupported,
        #[serde(rename = "unexpectedError")]
        UnexpectedError,
    }

    impl ::std::fmt::Display for DomainAvailabilityStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Available => f.write_str("available"),
                Self::Taken => f.write_str("taken"),
                Self::InvalidDomainName => f.write_str("invalidDomainName"),
                Self::TldNotSupported => f.write_str("tldNotSupported"),
                Self::UnexpectedError => f.write_str("unexpectedError"),
            }
        }
    }

    impl ::std::str::FromStr for DomainAvailabilityStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "available" => Ok(Self::Available),
                "taken" => Ok(Self::Taken),
                "invalidDomainName" => Ok(Self::InvalidDomainName),
                "tldNotSupported" => Ok(Self::TldNotSupported),
                "unexpectedError" => Ok(Self::UnexpectedError),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainAvailabilityStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DomainAvailabilityStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DomainAvailabilityStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`DomainClientEppStatus`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "clientDeleteProhibited",
    ///    "clientHold",
    ///    "clientRenewProhibited",
    ///    "clientTransferProhibited",
    ///    "clientUpdateProhibited"
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
    pub enum DomainClientEppStatus {
        #[serde(rename = "clientDeleteProhibited")]
        ClientDeleteProhibited,
        #[serde(rename = "clientHold")]
        ClientHold,
        #[serde(rename = "clientRenewProhibited")]
        ClientRenewProhibited,
        #[serde(rename = "clientTransferProhibited")]
        ClientTransferProhibited,
        #[serde(rename = "clientUpdateProhibited")]
        ClientUpdateProhibited,
    }

    impl ::std::fmt::Display for DomainClientEppStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::ClientDeleteProhibited => f.write_str("clientDeleteProhibited"),
                Self::ClientHold => f.write_str("clientHold"),
                Self::ClientRenewProhibited => f.write_str("clientRenewProhibited"),
                Self::ClientTransferProhibited => f.write_str("clientTransferProhibited"),
                Self::ClientUpdateProhibited => f.write_str("clientUpdateProhibited"),
            }
        }
    }

    impl ::std::str::FromStr for DomainClientEppStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "clientDeleteProhibited" => Ok(Self::ClientDeleteProhibited),
                "clientHold" => Ok(Self::ClientHold),
                "clientRenewProhibited" => Ok(Self::ClientRenewProhibited),
                "clientTransferProhibited" => Ok(Self::ClientTransferProhibited),
                "clientUpdateProhibited" => Ok(Self::ClientUpdateProhibited),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainClientEppStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DomainClientEppStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DomainClientEppStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Domain Contacts presented as ID references
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Domain Contacts presented as ID references",
    ///  "type": "object",
    ///  "required": [
    ///    "registrant"
    ///  ],
    ///  "properties": {
    ///    "admin": {
    ///      "description": "ID of the admin contact person",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/contactId"
    ///        }
    ///      ]
    ///    },
    ///    "attributes": {
    ///      "description": "List of extended attribute contact point IDs",
    ///      "examples": [
    ///        [
    ///          "1ZdMXpapqp9sle5dl8BlppTJXAzf3",
    ///          "1ZdMXpapqp9sle5dl8BlppTJXAzf2"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/contactId"
    ///      },
    ///      "maxItems": 5
    ///    },
    ///    "billing": {
    ///      "description": "ID of the billing contact person",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/contactId"
    ///        }
    ///      ]
    ///    },
    ///    "registrant": {
    ///      "description": "ID of the registrant contact person",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/contactId"
    ///        }
    ///      ]
    ///    },
    ///    "tech": {
    ///      "description": "ID of the technical contact person",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/contactId"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainContacts {
        ///ID of the admin contact person
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub admin: ::std::option::Option<ContactId>,
        ///List of extended attribute contact point IDs
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub attributes: ::std::vec::Vec<ContactId>,
        ///ID of the billing contact person
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub billing: ::std::option::Option<ContactId>,
        ///ID of the registrant contact person
        pub registrant: ContactId,
        ///ID of the technical contact person
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tech: ::std::option::Option<ContactId>,
    }

    ///`DomainCreateRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "autoRenew",
    ///    "contacts",
    ///    "privacyProtection",
    ///    "years"
    ///  ],
    ///  "properties": {
    ///    "autoRenew": {
    ///      "description": "Specifies whether automatic renewal will be enabled
    /// for the domain after the registration. After that, if autoRenew is set
    /// to true, the domain will be automatically renewed using the account’s
    /// default payment method upon each expiration",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "contacts": {
    ///      "description": "Domain contacts are specified using contact IDs.
    /// These contact IDs can be obtained from the “Get domain info” endpoint
    /// (/domains) for an existing domain, or by creating new contacts through
    /// the “Save contact details” endpoint (/contacts). Each contact ID
    /// corresponds to a specific contact person and includes their details
    /// (such as name, address, and email). These contacts will be associated
    /// with the domain during registration.",
    ///      "examples": [
    ///        {
    ///          "admin": "1ZdMXpapqp9sle5dl8BlppTJXAzf6",
    ///          "attributes": [
    ///            "1ZdMXpapqp9sle5dl8BlppTJXAzf8"
    ///          ],
    ///          "billing": "1ZdMXpapqp9sle5dl8BlppTJXAzf5",
    ///          "registrant": "1ZdMXpapqp9sle5dl8BlppTJXAzf5",
    ///          "tech": "1ZdMXpapqp9sle5dl8BlppTJXAzf5"
    ///        }
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/DomainContacts"
    ///        }
    ///      ]
    ///    },
    ///    "privacyProtection": {
    ///      "$ref": "#/components/schemas/DomainPrivacyOptions"
    ///    },
    ///    "years": {
    ///      "description": "Number of years to register the domain",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 10.0,
    ///      "minimum": 1.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainCreateRequest {
        ///Specifies whether automatic renewal will be enabled for the domain
        /// after the registration. After that, if autoRenew is set to true, the
        /// domain will be automatically renewed using the account’s default
        /// payment method upon each expiration
        #[serde(rename = "autoRenew")]
        pub auto_renew: bool,
        ///Domain contacts are specified using contact IDs. These contact IDs
        /// can be obtained from the “Get domain info” endpoint (/domains) for
        /// an existing domain, or by creating new contacts through the “Save
        /// contact details” endpoint (/contacts). Each contact ID corresponds
        /// to a specific contact person and includes their details (such as
        /// name, address, and email). These contacts will be associated with
        /// the domain during registration.
        pub contacts: DomainContacts,
        #[serde(rename = "privacyProtection")]
        pub privacy_protection: DomainPrivacyOptions,
        ///Number of years to register the domain
        pub years: ::std::num::NonZeroU32,
    }

    ///Domain description text
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Domain description text",
    ///  "examples": [
    ///    "Premium domain for sale"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 5000,
    ///  "pattern": "^[\\s\\S]*$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct DomainDescription(::std::string::String);
    impl ::std::ops::Deref for DomainDescription {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DomainDescription> for ::std::string::String {
        fn from(value: DomainDescription) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for DomainDescription {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5000usize {
                return Err("longer than 5000 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[\\s\\S]*$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\s\\S]*$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainDescription {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DomainDescription {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DomainDescription {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for DomainDescription {
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

    ///`DomainInfo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "autoRenew",
    ///    "contacts",
    ///    "eppStatuses",
    ///    "expirationDate",
    ///    "isPremium",
    ///    "lifecycleStatus",
    ///    "name",
    ///    "nameservers",
    ///    "privacyProtection",
    ///    "registrationDate",
    ///    "suspensions",
    ///    "unicodeName",
    ///    "verificationStatus"
    ///  ],
    ///  "properties": {
    ///    "autoRenew": {
    ///      "description": "Indicates whether the auto-renew option is
    /// enabled",
    ///      "readOnly": true,
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "contacts": {
    ///      "examples": [
    ///        {
    ///          "admin": "1ZdMXpapqp9sle5dl8BlppTJXAzf6",
    ///          "attributes": [
    ///            "1ZdMXpapqp9sle5dl8BlppTJXAzf8"
    ///          ],
    ///          "billing": "1ZdMXpapqp9sle5dl8BlppTJXAzf5",
    ///          "registrant": "1ZdMXpapqp9sle5dl8BlppTJXAzf5",
    ///          "tech": "1ZdMXpapqp9sle5dl8BlppTJXAzf5"
    ///        }
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/DomainContacts"
    ///        }
    ///      ]
    ///    },
    ///    "eppStatuses": {
    ///      "readOnly": true,
    ///      "examples": [
    ///        [
    ///          "clientTransferProhibited"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/DomainClientEPPStatus"
    ///      }
    ///    },
    ///    "expirationDate": {
    ///      "description": "The date when the domain registration expires",
    ///      "readOnly": true,
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/isoDate"
    ///        }
    ///      ]
    ///    },
    ///    "isPremium": {
    ///      "description": "Indicates whether the domain is premium",
    ///      "readOnly": true,
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "lifecycleStatus": {
    ///      "description": "Current lifecycle status of the domain. Possible
    /// values are:\n\n* `creating` - The domain is being registered. This
    /// status means that the request has not yet been processed by the
    /// registry.\n* `registered` - The domain is registered.\n* `grace1` - The
    /// domain has expired but is still fully manageable.\n* `grace2` - The
    /// domain expired, parked, and its management is limited.\n* `redemption` -
    /// The domain has expired and may not be recoverable. If restoration is
    /// possible, an additional fee may apply.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "registered"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/DomainLifecycleStatus"
    ///        }
    ///      ]
    ///    },
    ///    "name": {
    ///      "examples": [
    ///        "xn--spceship-9ya.com"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/domainNameALabel"
    ///        }
    ///      ]
    ///    },
    ///    "nameservers": {
    ///      "description": "Information about nameservers",
    ///      "examples": [
    ///        {
    ///          "hosts": [
    ///            "ns1.exampledomain.com",
    ///            "ns2.exampledomain.com"
    ///          ],
    ///          "provider": "basic"
    ///        }
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref":
    /// "#/components/schemas/DomainNameServersConfigurationResponse"
    ///        }
    ///      ]
    ///    },
    ///    "privacyProtection": {
    ///      "description": "Information about domain privacy protection",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/DomainPrivacyProtection"
    ///        }
    ///      ]
    ///    },
    ///    "registrationDate": {
    ///      "description": "The date when the domain was registered",
    ///      "readOnly": true,
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/isoDate"
    ///        }
    ///      ]
    ///    },
    ///    "suspensions": {
    ///      "description": "Information about domain suspensions.\nThe array
    /// may be empty, contain details about one suspension (raaVerification,
    /// abuse, promoAbuse, fraud, pendingAccountVerification,
    /// unauthorizedAccess, tosViolation, transferDispute, restrictedSecurity,
    /// lockCourt, suspendCourt, udrpUrs, restrictedLegal, paymentPending,
    /// unpaidService, restrictedWhois, or lockedWhois), or up to two: one
    /// raaVerification and one legal (abuse, promoAbuse, fraud,
    /// pendingAccountVerification, unauthorizedAccess, tosViolation,
    /// transferDispute, restrictedSecurity, lockCourt, suspendCourt, udrpUrs,
    /// restrictedLegal, paymentPending, unpaidService, restrictedWhois, or
    /// lockedWhois).",
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/DomainSuspensionDetails"
    ///      },
    ///      "maxItems": 2
    ///    },
    ///    "unicodeName": {
    ///      "examples": [
    ///        "spaceship.com"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/domainNameULabel"
    ///        }
    ///      ]
    ///    },
    ///    "verificationStatus": {
    ///      "description": "Status of the RAA verification process. `null` if
    /// RAA procedure is not applied for the domain.",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "success"
    ///      ],
    ///      "oneOf": [
    ///        {
    ///          "type": "null"
    ///        },
    ///        {
    ///          "allOf": [
    ///            {
    ///              "$ref": "#/components/schemas/DomainVerificationStatus"
    ///            }
    ///          ]
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainInfo {
        ///Indicates whether the auto-renew option is enabled
        #[serde(rename = "autoRenew")]
        pub auto_renew: bool,
        pub contacts: DomainContacts,
        #[serde(rename = "eppStatuses")]
        pub epp_statuses: ::std::vec::Vec<DomainClientEppStatus>,
        ///The date when the domain registration expires
        #[serde(rename = "expirationDate")]
        pub expiration_date: IsoDate,
        ///Indicates whether the domain is premium
        #[serde(rename = "isPremium")]
        pub is_premium: bool,
        ///Current lifecycle status of the domain. Possible values are:
        ///
        /// * `creating` - The domain is being registered. This status means
        ///   that the request has not yet been processed by the registry.
        /// * `registered` - The domain is registered.
        /// * `grace1` - The domain has expired but is still fully manageable.
        /// * `grace2` - The domain expired, parked, and its management is
        ///   limited.
        /// * `redemption` - The domain has expired and may not be recoverable.
        ///   If restoration is possible, an additional fee may apply.
        #[serde(rename = "lifecycleStatus")]
        pub lifecycle_status: DomainLifecycleStatus,
        pub name: DomainNameALabel,
        ///Information about nameservers
        pub nameservers: DomainNameServersConfigurationResponse,
        ///Information about domain privacy protection
        #[serde(rename = "privacyProtection")]
        pub privacy_protection: DomainPrivacyProtection,
        ///The date when the domain was registered
        #[serde(rename = "registrationDate")]
        pub registration_date: IsoDate,
        ///Information about domain suspensions.
        ///The array may be empty, contain details about one suspension
        /// (raaVerification, abuse, promoAbuse, fraud,
        /// pendingAccountVerification, unauthorizedAccess, tosViolation,
        /// transferDispute, restrictedSecurity, lockCourt, suspendCourt,
        /// udrpUrs, restrictedLegal, paymentPending, unpaidService,
        /// restrictedWhois, or lockedWhois), or up to two: one raaVerification
        /// and one legal (abuse, promoAbuse, fraud, pendingAccountVerification,
        /// unauthorizedAccess, tosViolation, transferDispute,
        /// restrictedSecurity, lockCourt, suspendCourt, udrpUrs,
        /// restrictedLegal, paymentPending, unpaidService, restrictedWhois, or
        /// lockedWhois).
        pub suspensions: ::std::vec::Vec<DomainSuspensionDetails>,
        #[serde(rename = "unicodeName")]
        pub unicode_name: DomainNameULabel,
        ///Status of the RAA verification process. `null` if RAA procedure is
        /// not applied for the domain.
        #[serde(rename = "verificationStatus")]
        pub verification_status: ::std::option::Option<DomainVerificationStatus>,
    }

    ///`DomainLifecycleStatus`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "creating",
    ///    "registered",
    ///    "grace1",
    ///    "grace2",
    ///    "redemption"
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
    pub enum DomainLifecycleStatus {
        #[serde(rename = "creating")]
        Creating,
        #[serde(rename = "registered")]
        Registered,
        #[serde(rename = "grace1")]
        Grace1,
        #[serde(rename = "grace2")]
        Grace2,
        #[serde(rename = "redemption")]
        Redemption,
    }

    impl ::std::fmt::Display for DomainLifecycleStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Creating => f.write_str("creating"),
                Self::Registered => f.write_str("registered"),
                Self::Grace1 => f.write_str("grace1"),
                Self::Grace2 => f.write_str("grace2"),
                Self::Redemption => f.write_str("redemption"),
            }
        }
    }

    impl ::std::str::FromStr for DomainLifecycleStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "creating" => Ok(Self::Creating),
                "registered" => Ok(Self::Registered),
                "grace1" => Ok(Self::Grace1),
                "grace2" => Ok(Self::Grace2),
                "redemption" => Ok(Self::Redemption),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainLifecycleStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DomainLifecycleStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DomainLifecycleStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Domain name in UTF-8 or ASCII format (U-label or A-label)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Domain name in UTF-8 or ASCII format (U-label or
    /// A-label)",
    ///  "examples": [
    ///    "spaceship.com"
    ///  ],
    ///  "type": "string",
    ///  "format": "domain",
    ///  "maxLength": 255,
    ///  "minLength": 4
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
    pub struct DomainName(pub ::std::string::String);
    impl ::std::ops::Deref for DomainName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DomainName> for ::std::string::String {
        fn from(value: DomainName) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::string::String> for DomainName {
        fn from(value: ::std::string::String) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DomainName {
        type Err = ::std::convert::Infallible;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::fmt::Display for DomainName {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Domain name in ASCII format (A-label)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Domain name in ASCII format (A-label)",
    ///  "examples": [
    ///    "xn--spceship-9ya.com"
    ///  ],
    ///  "type": "string",
    ///  "format": "domain",
    ///  "maxLength": 255,
    ///  "minLength": 4
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
    pub struct DomainNameALabel(pub ::std::string::String);
    impl ::std::ops::Deref for DomainNameALabel {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DomainNameALabel> for ::std::string::String {
        fn from(value: DomainNameALabel) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::string::String> for DomainNameALabel {
        fn from(value: ::std::string::String) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DomainNameALabel {
        type Err = ::std::convert::Infallible;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::fmt::Display for DomainNameALabel {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///`DomainNameServersConfigurationRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "provider"
    ///  ],
    ///  "properties": {
    ///    "hosts": {
    ///      "description": "A list of nameservers to be assigned to the domain.
    /// Each nameserver must be provided in a fully qualified domain
    /// format.\nThis field must be specified only for the \"custom\" provider;
    /// for the \"basic\" provider it should be omitted.",
    ///      "examples": [
    ///        [
    ///          "ns1.exampledomain.com",
    ///          "ns2.exampledomain.com"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/fqdn"
    ///      },
    ///      "maxItems": 12,
    ///      "minItems": 2
    ///    },
    ///    "provider": {
    ///      "description": "Nameservers provider",
    ///      "examples": [
    ///        "custom"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/Provider"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainNameServersConfigurationRequest {
        ///A list of nameservers to be assigned to the domain. Each nameserver
        /// must be provided in a fully qualified domain format.
        /// This field must be specified only for the "custom" provider; for the
        /// "basic" provider it should be omitted.
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub hosts: ::std::vec::Vec<Fqdn>,
        ///Nameservers provider
        pub provider: Provider,
    }

    ///`DomainNameServersConfigurationResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref":
    /// "#/components/schemas/DomainNameServersConfigurationRequest"
    ///    }
    ///  ],
    ///  "required": [
    ///    "hosts"
    ///  ],
    ///  "properties": {
    ///    "hosts": {
    ///      "description": "A list of nameservers assigned to the domain. Each
    /// nameserver is provided in a fully qualified domain format.",
    ///      "examples": [
    ///        [
    ///          "ns1.exampledomain.com",
    ///          "ns2.exampledomain.com"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/fqdn"
    ///      },
    ///      "maxItems": 12,
    ///      "minItems": 2
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainNameServersConfigurationResponse {
        pub hosts: ::std::vec::Vec<Fqdn>,
        ///Nameservers provider
        pub provider: Provider,
    }

    ///Domain name in UTF-8 format (U-label)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Domain name in UTF-8 format (U-label)",
    ///  "examples": [
    ///    "spaceship.com"
    ///  ],
    ///  "type": "string",
    ///  "format": "domain",
    ///  "maxLength": 255,
    ///  "minLength": 4
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
    pub struct DomainNameULabel(pub ::std::string::String);
    impl ::std::ops::Deref for DomainNameULabel {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<DomainNameULabel> for ::std::string::String {
        fn from(value: DomainNameULabel) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::string::String> for DomainNameULabel {
        fn from(value: ::std::string::String) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for DomainNameULabel {
        type Err = ::std::convert::Infallible;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::fmt::Display for DomainNameULabel {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Domain premium prices
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Domain premium prices",
    ///  "type": "object",
    ///  "required": [
    ///    "currency",
    ///    "operation",
    ///    "price"
    ///  ],
    ///  "properties": {
    ///    "currency": {
    ///      "description": "Premium price currency",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/Currency"
    ///        }
    ///      ]
    ///    },
    ///    "operation": {
    ///      "description": "Operation to which a premium price is applied",
    ///      "examples": [
    ///        "register"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "register",
    ///        "transfer",
    ///        "renew",
    ///        "restore"
    ///      ]
    ///    },
    ///    "price": {
    ///      "description": "Premium price amount",
    ///      "examples": [
    ///        10.99
    ///      ],
    ///      "type": "number",
    ///      "format": "decimal"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainPriceDetails {
        ///Premium price currency
        pub currency: Currency,
        ///Operation to which a premium price is applied
        pub operation: DomainPriceDetailsOperation,
        ///Premium price amount
        pub price: f64,
    }

    ///Operation to which a premium price is applied
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Operation to which a premium price is applied",
    ///  "examples": [
    ///    "register"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "register",
    ///    "transfer",
    ///    "renew",
    ///    "restore"
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
    pub enum DomainPriceDetailsOperation {
        #[serde(rename = "register")]
        Register,
        #[serde(rename = "transfer")]
        Transfer,
        #[serde(rename = "renew")]
        Renew,
        #[serde(rename = "restore")]
        Restore,
    }

    impl ::std::fmt::Display for DomainPriceDetailsOperation {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Register => f.write_str("register"),
                Self::Transfer => f.write_str("transfer"),
                Self::Renew => f.write_str("renew"),
                Self::Restore => f.write_str("restore"),
            }
        }
    }

    impl ::std::str::FromStr for DomainPriceDetailsOperation {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "register" => Ok(Self::Register),
                "transfer" => Ok(Self::Transfer),
                "renew" => Ok(Self::Renew),
                "restore" => Ok(Self::Restore),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainPriceDetailsOperation {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DomainPriceDetailsOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DomainPriceDetailsOperation {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`DomainPrivacyLevel`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "public",
    ///    "high"
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
    pub enum DomainPrivacyLevel {
        #[serde(rename = "public")]
        Public,
        #[serde(rename = "high")]
        High,
    }

    impl ::std::fmt::Display for DomainPrivacyLevel {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Public => f.write_str("public"),
                Self::High => f.write_str("high"),
            }
        }
    }

    impl ::std::str::FromStr for DomainPrivacyLevel {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "public" => Ok(Self::Public),
                "high" => Ok(Self::High),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainPrivacyLevel {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DomainPrivacyLevel {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DomainPrivacyLevel {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Privacy protection options. This parameter controls the visibility of
    /// your domain registrant contact information in the public WHOIS database.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Privacy protection options. This parameter controls the
    /// visibility of your domain registrant contact information in the public
    /// WHOIS database.",
    ///  "type": "object",
    ///  "required": [
    ///    "level",
    ///    "userConsent"
    ///  ],
    ///  "properties": {
    ///    "level": {
    ///      "description": "Indicates the level of privacy protection. Set this
    /// property to \"high\" to enable privacy protection and hide your personal
    /// details, or to \"public\" to display your real contact
    /// information.\nWhen choosing the \"public\" level, the \"userConsent\"
    /// property must be set to true to confirm your explicit consent for making
    /// your contact information publicly available.\n\nNote: not all TLDs
    /// support \"high\" privacy protection level",
    ///      "examples": [
    ///        "high"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/DomainPrivacyLevel"
    ///        }
    ///      ]
    ///    },
    ///    "userConsent": {
    ///      "description": "Indicates whether the user consents to the Public
    /// privacy settings",
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
    pub struct DomainPrivacyOptions {
        ///Indicates the level of privacy protection. Set this property to
        /// "high" to enable privacy protection and hide your personal details,
        /// or to "public" to display your real contact information.
        /// When choosing the "public" level, the "userConsent" property must be
        /// set to true to confirm your explicit consent for making your contact
        /// information publicly available.
        ///
        ///Note: not all TLDs support "high" privacy protection level
        pub level: DomainPrivacyLevel,
        ///Indicates whether the user consents to the Public privacy settings
        #[serde(rename = "userConsent")]
        pub user_consent: bool,
    }

    ///`DomainPrivacyProtection`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "contactForm",
    ///    "level"
    ///  ],
    ///  "properties": {
    ///    "contactForm": {
    ///      "description": "Indicates whether WHOIS should display the contact
    /// form link",
    ///      "examples": [
    ///        true
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "level": {
    ///      "description": "Level of privacy protection set for the domain",
    ///      "examples": [
    ///        "high"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/DomainPrivacyLevel"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainPrivacyProtection {
        ///Indicates whether WHOIS should display the contact form link
        #[serde(rename = "contactForm")]
        pub contact_form: bool,
        ///Level of privacy protection set for the domain
        pub level: DomainPrivacyLevel,
    }

    ///`DomainSuspensionDetails`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "reasonCode"
    ///  ],
    ///  "properties": {
    ///    "reasonCode": {
    ///      "description": "Suspension reason code",
    ///      "examples": [
    ///        "raaVerification"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "raaVerification",
    ///        "abuse",
    ///        "promoAbuse",
    ///        "fraud",
    ///        "pendingAccountVerification",
    ///        "unauthorizedAccess",
    ///        "tosViolation",
    ///        "transferDispute",
    ///        "restrictedSecurity",
    ///        "lockCourt",
    ///        "suspendCourt",
    ///        "udrpUrs",
    ///        "restrictedLegal",
    ///        "paymentPending",
    ///        "unpaidService",
    ///        "restrictedWhois",
    ///        "lockedWhois"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainSuspensionDetails {
        ///Suspension reason code
        #[serde(rename = "reasonCode")]
        pub reason_code: DomainSuspensionDetailsReasonCode,
    }

    ///Suspension reason code
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Suspension reason code",
    ///  "examples": [
    ///    "raaVerification"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "raaVerification",
    ///    "abuse",
    ///    "promoAbuse",
    ///    "fraud",
    ///    "pendingAccountVerification",
    ///    "unauthorizedAccess",
    ///    "tosViolation",
    ///    "transferDispute",
    ///    "restrictedSecurity",
    ///    "lockCourt",
    ///    "suspendCourt",
    ///    "udrpUrs",
    ///    "restrictedLegal",
    ///    "paymentPending",
    ///    "unpaidService",
    ///    "restrictedWhois",
    ///    "lockedWhois"
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
    pub enum DomainSuspensionDetailsReasonCode {
        #[serde(rename = "raaVerification")]
        RaaVerification,
        #[serde(rename = "abuse")]
        Abuse,
        #[serde(rename = "promoAbuse")]
        PromoAbuse,
        #[serde(rename = "fraud")]
        Fraud,
        #[serde(rename = "pendingAccountVerification")]
        PendingAccountVerification,
        #[serde(rename = "unauthorizedAccess")]
        UnauthorizedAccess,
        #[serde(rename = "tosViolation")]
        TosViolation,
        #[serde(rename = "transferDispute")]
        TransferDispute,
        #[serde(rename = "restrictedSecurity")]
        RestrictedSecurity,
        #[serde(rename = "lockCourt")]
        LockCourt,
        #[serde(rename = "suspendCourt")]
        SuspendCourt,
        #[serde(rename = "udrpUrs")]
        UdrpUrs,
        #[serde(rename = "restrictedLegal")]
        RestrictedLegal,
        #[serde(rename = "paymentPending")]
        PaymentPending,
        #[serde(rename = "unpaidService")]
        UnpaidService,
        #[serde(rename = "restrictedWhois")]
        RestrictedWhois,
        #[serde(rename = "lockedWhois")]
        LockedWhois,
    }

    impl ::std::fmt::Display for DomainSuspensionDetailsReasonCode {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::RaaVerification => f.write_str("raaVerification"),
                Self::Abuse => f.write_str("abuse"),
                Self::PromoAbuse => f.write_str("promoAbuse"),
                Self::Fraud => f.write_str("fraud"),
                Self::PendingAccountVerification => f.write_str("pendingAccountVerification"),
                Self::UnauthorizedAccess => f.write_str("unauthorizedAccess"),
                Self::TosViolation => f.write_str("tosViolation"),
                Self::TransferDispute => f.write_str("transferDispute"),
                Self::RestrictedSecurity => f.write_str("restrictedSecurity"),
                Self::LockCourt => f.write_str("lockCourt"),
                Self::SuspendCourt => f.write_str("suspendCourt"),
                Self::UdrpUrs => f.write_str("udrpUrs"),
                Self::RestrictedLegal => f.write_str("restrictedLegal"),
                Self::PaymentPending => f.write_str("paymentPending"),
                Self::UnpaidService => f.write_str("unpaidService"),
                Self::RestrictedWhois => f.write_str("restrictedWhois"),
                Self::LockedWhois => f.write_str("lockedWhois"),
            }
        }
    }

    impl ::std::str::FromStr for DomainSuspensionDetailsReasonCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "raaVerification" => Ok(Self::RaaVerification),
                "abuse" => Ok(Self::Abuse),
                "promoAbuse" => Ok(Self::PromoAbuse),
                "fraud" => Ok(Self::Fraud),
                "pendingAccountVerification" => Ok(Self::PendingAccountVerification),
                "unauthorizedAccess" => Ok(Self::UnauthorizedAccess),
                "tosViolation" => Ok(Self::TosViolation),
                "transferDispute" => Ok(Self::TransferDispute),
                "restrictedSecurity" => Ok(Self::RestrictedSecurity),
                "lockCourt" => Ok(Self::LockCourt),
                "suspendCourt" => Ok(Self::SuspendCourt),
                "udrpUrs" => Ok(Self::UdrpUrs),
                "restrictedLegal" => Ok(Self::RestrictedLegal),
                "paymentPending" => Ok(Self::PaymentPending),
                "unpaidService" => Ok(Self::UnpaidService),
                "restrictedWhois" => Ok(Self::RestrictedWhois),
                "lockedWhois" => Ok(Self::LockedWhois),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainSuspensionDetailsReasonCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DomainSuspensionDetailsReasonCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DomainSuspensionDetailsReasonCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`DomainTransferRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "autoRenew",
    ///    "contacts",
    ///    "privacyProtection"
    ///  ],
    ///  "properties": {
    ///    "authCode": {
    ///      "description": "Authorization code (EPP code) required for domain
    /// transfers. This code is mandatory for most TLDs but may be optional for
    /// certain ccTLDs, for example, the .uk.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/authCode"
    ///        }
    ///      ]
    ///    },
    ///    "autoRenew": {
    ///      "description": "Specifies whether automatic renewal will be enabled
    /// for the domain after the transfer is completed. After that, if autoRenew
    /// is set to true, the domain will be automatically renewed using the
    /// account’s default payment method upon each expiration",
    ///      "examples": [
    ///        false
    ///      ],
    ///      "type": "boolean"
    ///    },
    ///    "contacts": {
    ///      "description": "Domain contacts are specified using contact IDs.
    /// These contact IDs can be obtained from the “Get domain info” endpoint
    /// (/domains) for an existing domain, or by creating new contacts through
    /// the “Save contact details” endpoint (/contacts). Each contact ID
    /// corresponds to a specific contact person and includes their details
    /// (such as name, address, and email). These contacts will be associated
    /// with the domain after transfer.",
    ///      "examples": [
    ///        {
    ///          "admin": "1ZdMXpapqp9sle5dl8BlppTJXAzf6",
    ///          "attributes": [
    ///            "1ZdMXpapqp9sle5dl8BlppTJXAzf8"
    ///          ],
    ///          "billing": "1ZdMXpapqp9sle5dl8BlppTJXAzf5",
    ///          "registrant": "1ZdMXpapqp9sle5dl8BlppTJXAzf5",
    ///          "tech": "1ZdMXpapqp9sle5dl8BlppTJXAzf5"
    ///        }
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/DomainContacts"
    ///        }
    ///      ]
    ///    },
    ///    "privacyProtection": {
    ///      "$ref": "#/components/schemas/DomainPrivacyOptions"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainTransferRequest {
        ///Authorization code (EPP code) required for domain transfers. This
        /// code is mandatory for most TLDs but may be optional for certain
        /// ccTLDs, for example, the .uk.
        #[serde(
            rename = "authCode",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub auth_code: ::std::option::Option<AuthCode>,
        ///Specifies whether automatic renewal will be enabled for the domain
        /// after the transfer is completed. After that, if autoRenew is set to
        /// true, the domain will be automatically renewed using the account’s
        /// default payment method upon each expiration
        #[serde(rename = "autoRenew")]
        pub auto_renew: bool,
        ///Domain contacts are specified using contact IDs. These contact IDs
        /// can be obtained from the “Get domain info” endpoint (/domains) for
        /// an existing domain, or by creating new contacts through the “Save
        /// contact details” endpoint (/contacts). Each contact ID corresponds
        /// to a specific contact person and includes their details (such as
        /// name, address, and email). These contacts will be associated with
        /// the domain after transfer.
        pub contacts: DomainContacts,
        #[serde(rename = "privacyProtection")]
        pub privacy_protection: DomainPrivacyOptions,
    }

    ///Status of the domain name transfer. Possible values are:
    /// * `pending` - transfer is ongoing
    /// * `completed` - transfer was completed
    /// * `cancelled` - transfer request was cancelled by loosing party
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Status of the domain name transfer. Possible values
    /// are:\n* `pending` - transfer is ongoing\n* `completed` - transfer was
    /// completed\n* `cancelled` - transfer request was cancelled by loosing
    /// party",
    ///  "type": "string",
    ///  "enum": [
    ///    "pending",
    ///    "completed",
    ///    "cancelled"
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
    pub enum DomainTransferStatus {
        #[serde(rename = "pending")]
        Pending,
        #[serde(rename = "completed")]
        Completed,
        #[serde(rename = "cancelled")]
        Cancelled,
    }

    impl ::std::fmt::Display for DomainTransferStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Pending => f.write_str("pending"),
                Self::Completed => f.write_str("completed"),
                Self::Cancelled => f.write_str("cancelled"),
            }
        }
    }

    impl ::std::str::FromStr for DomainTransferStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "pending" => Ok(Self::Pending),
                "completed" => Ok(Self::Completed),
                "cancelled" => Ok(Self::Cancelled),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainTransferStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DomainTransferStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DomainTransferStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Status of the RAA verification process. Possible values are:
    /// * `verification` - Verification process is in progress and requires user
    ///   interaction.
    /// * `success` - Verification success.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Status of the RAA verification process. Possible values
    /// are:\n* `verification` - Verification process is in progress and
    /// requires user interaction.\n* `success` - Verification success.",
    ///  "examples": [
    ///    "success"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "verification",
    ///    "success"
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
    pub enum DomainValidVerificationStatus {
        #[serde(rename = "verification")]
        Verification,
        #[serde(rename = "success")]
        Success,
    }

    impl ::std::fmt::Display for DomainValidVerificationStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Verification => f.write_str("verification"),
                Self::Success => f.write_str("success"),
            }
        }
    }

    impl ::std::str::FromStr for DomainValidVerificationStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "verification" => Ok(Self::Verification),
                "success" => Ok(Self::Success),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainValidVerificationStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DomainValidVerificationStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DomainValidVerificationStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Status of the RAA verification process. Possible values are:
    /// * `verification` - Verification process is in progress and requires user
    ///   interaction.
    /// * `success` - Verification success.
    /// * `failed` - Verification failed.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Status of the RAA verification process. Possible values
    /// are:\n* `verification` - Verification process is in progress and
    /// requires user interaction.\n* `success` - Verification success.\n*
    /// `failed` - Verification failed.",
    ///  "examples": [
    ///    "success"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "verification",
    ///    "success",
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
    pub enum DomainVerificationStatus {
        #[serde(rename = "verification")]
        Verification,
        #[serde(rename = "success")]
        Success,
        #[serde(rename = "failed")]
        Failed,
    }

    impl ::std::fmt::Display for DomainVerificationStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Verification => f.write_str("verification"),
                Self::Success => f.write_str("success"),
                Self::Failed => f.write_str("failed"),
            }
        }
    }

    impl ::std::str::FromStr for DomainVerificationStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "verification" => Ok(Self::Verification),
                "success" => Ok(Self::Success),
                "failed" => Ok(Self::Failed),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainVerificationStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for DomainVerificationStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DomainVerificationStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Represents wrapper object for auth code response
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Represents wrapper object for auth code response",
    ///  "type": "object",
    ///  "required": [
    ///    "authCode",
    ///    "expires"
    ///  ],
    ///  "properties": {
    ///    "authCode": {
    ///      "$ref": "#/components/schemas/authCode"
    ///    },
    ///    "expires": {
    ///      "description": "The expiration date for the auth code",
    ///      "readOnly": true,
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/isoDate"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainsDomainAuthCodeResponse {
        #[serde(rename = "authCode")]
        pub auth_code: AuthCode,
        ///The expiration date for the auth code
        pub expires: IsoDate,
    }

    ///Represents wrapper object for autorenewal state
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Represents wrapper object for autorenewal state",
    ///  "type": "object",
    ///  "required": [
    ///    "isEnabled"
    ///  ],
    ///  "properties": {
    ///    "isEnabled": {
    ///      "description": "Describes autorenewal state for the domain",
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
    pub struct DomainsDomainAutoRenewal {
        ///Describes autorenewal state for the domain
        #[serde(rename = "isEnabled")]
        pub is_enabled: bool,
    }

    ///Represents a wrapper object for domain's email protection preference
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Represents a wrapper object for domain's email
    /// protection preference",
    ///  "type": "object",
    ///  "required": [
    ///    "contactForm"
    ///  ],
    ///  "properties": {
    ///    "contactForm": {
    ///      "description": "Indicates whether WHOIS should display the contact
    /// form link",
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
    pub struct DomainsDomainEmailProtectionPreference {
        ///Indicates whether WHOIS should display the contact form link
        #[serde(rename = "contactForm")]
        pub contact_form: bool,
    }

    ///Represents a wrapper object for domain's privacy level preference
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Represents a wrapper object for domain's privacy level
    /// preference",
    ///  "type": "object",
    ///  "required": [
    ///    "privacyLevel",
    ///    "userConsent"
    ///  ],
    ///  "properties": {
    ///    "privacyLevel": {
    ///      "description": "Describes privacy preference for the domain",
    ///      "examples": [
    ///        "high"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/DomainPrivacyLevel"
    ///        }
    ///      ]
    ///    },
    ///    "userConsent": {
    ///      "description": "Expresses the user's consent for privacy changes.
    /// The operation will be performed ONLY if the flag is 'true'.",
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
    pub struct DomainsDomainPrivacyPreference {
        ///Describes privacy preference for the domain
        #[serde(rename = "privacyLevel")]
        pub privacy_level: DomainPrivacyLevel,
        ///Expresses the user's consent for privacy changes. The operation will
        /// be performed ONLY if the flag is 'true'.
        #[serde(rename = "userConsent")]
        pub user_consent: bool,
    }

    ///Parameters required to request a domain renewal
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Parameters required to request a domain renewal",
    ///  "type": "object",
    ///  "required": [
    ///    "currentExpirationDate",
    ///    "years"
    ///  ],
    ///  "properties": {
    ///    "currentExpirationDate": {
    ///      "description": "Current expiration date",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/isoDate"
    ///        }
    ///      ]
    ///    },
    ///    "years": {
    ///      "description": "Renewal years",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 10.0,
    ///      "minimum": 1.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainsDomainRenewalRequestInfo {
        ///Current expiration date
        #[serde(rename = "currentExpirationDate")]
        pub current_expiration_date: IsoDate,
        ///Renewal years
        pub years: ::std::num::NonZeroU32,
    }

    ///Details of the domain name transfer
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Details of the domain name transfer",
    ///  "type": "object",
    ///  "required": [
    ///    "direction",
    ///    "startedAt",
    ///    "status"
    ///  ],
    ///  "properties": {
    ///    "direction": {
    ///      "description": "Transfer direction: incoming",
    ///      "type": "string",
    ///      "enum": [
    ///        "in"
    ///      ]
    ///    },
    ///    "finishedAt": {
    ///      "description": "Date when transfer was finished",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/isoDate"
    ///        }
    ///      ]
    ///    },
    ///    "startedAt": {
    ///      "description": "Date when transfer was initiated",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/isoDate"
    ///        }
    ///      ]
    ///    },
    ///    "status": {
    ///      "readOnly": true,
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/DomainTransferStatus"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainsDomainTransferDetailsResponse {
        ///Transfer direction: incoming
        pub direction: DomainsDomainTransferDetailsResponseDirection,
        ///Date when transfer was finished
        #[serde(
            rename = "finishedAt",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub finished_at: ::std::option::Option<IsoDate>,
        ///Date when transfer was initiated
        #[serde(rename = "startedAt")]
        pub started_at: IsoDate,
        pub status: DomainTransferStatus,
    }

    ///Transfer direction: incoming
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Transfer direction: incoming",
    ///  "type": "string",
    ///  "enum": [
    ///    "in"
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
    pub enum DomainsDomainTransferDetailsResponseDirection {
        #[serde(rename = "in")]
        In,
    }

    impl ::std::fmt::Display for DomainsDomainTransferDetailsResponseDirection {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::In => f.write_str("in"),
            }
        }
    }

    impl ::std::str::FromStr for DomainsDomainTransferDetailsResponseDirection {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "in" => Ok(Self::In),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainsDomainTransferDetailsResponseDirection {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for DomainsDomainTransferDetailsResponseDirection
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for DomainsDomainTransferDetailsResponseDirection
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Represents wrapper object for transfer lock
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Represents wrapper object for transfer lock",
    ///  "type": "object",
    ///  "required": [
    ///    "isLocked"
    ///  ],
    ///  "properties": {
    ///    "isLocked": {
    ///      "description": "Describes transfer lock for the domain",
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
    pub struct DomainsDomainTransferLock {
        ///Describes transfer lock for the domain
        #[serde(rename = "isLocked")]
        pub is_locked: bool,
    }

    ///Domains list query params
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Domains list query params",
    ///  "type": "object",
    ///  "required": [
    ///    "skip",
    ///    "take"
    ///  ],
    ///  "properties": {
    ///    "orderBy": {
    ///      "description": "Specifies fields and order to sort the response
    /// items",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "enum": [
    ///          "name",
    ///          "-name",
    ///          "unicodeName",
    ///          "-unicodeName",
    ///          "registrationDate",
    ///          "-registrationDate",
    ///          "expirationDate",
    ///          "-expirationDate"
    ///        ]
    ///      },
    ///      "maxItems": 1
    ///    },
    ///    "skip": {
    ///      "description": "Number of response items to skip",
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 2147483647.0,
    ///      "minimum": 0.0
    ///    },
    ///    "take": {
    ///      "description": "Number of response items per page",
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 100.0,
    ///      "minimum": 1.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainsGetDomainListQueryParams {
        ///Specifies fields and order to sort the response items
        #[serde(
            rename = "orderBy",
            default,
            skip_serializing_if = "::std::vec::Vec::is_empty"
        )]
        pub order_by: ::std::vec::Vec<DomainsGetDomainListQueryParamsOrderByItem>,
        ///Number of response items to skip
        pub skip: i32,
        ///Number of response items per page
        pub take: ::std::num::NonZeroU32,
    }

    ///`DomainsGetDomainListQueryParamsOrderByItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "name",
    ///    "-name",
    ///    "unicodeName",
    ///    "-unicodeName",
    ///    "registrationDate",
    ///    "-registrationDate",
    ///    "expirationDate",
    ///    "-expirationDate"
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
    pub enum DomainsGetDomainListQueryParamsOrderByItem {
        #[serde(rename = "name")]
        Name,
        #[serde(rename = "-name")]
        Xname,
        #[serde(rename = "unicodeName")]
        UnicodeName,
        #[serde(rename = "-unicodeName")]
        XunicodeName,
        #[serde(rename = "registrationDate")]
        RegistrationDate,
        #[serde(rename = "-registrationDate")]
        XregistrationDate,
        #[serde(rename = "expirationDate")]
        ExpirationDate,
        #[serde(rename = "-expirationDate")]
        XexpirationDate,
    }

    impl ::std::fmt::Display for DomainsGetDomainListQueryParamsOrderByItem {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Name => f.write_str("name"),
                Self::Xname => f.write_str("-name"),
                Self::UnicodeName => f.write_str("unicodeName"),
                Self::XunicodeName => f.write_str("-unicodeName"),
                Self::RegistrationDate => f.write_str("registrationDate"),
                Self::XregistrationDate => f.write_str("-registrationDate"),
                Self::ExpirationDate => f.write_str("expirationDate"),
                Self::XexpirationDate => f.write_str("-expirationDate"),
            }
        }
    }

    impl ::std::str::FromStr for DomainsGetDomainListQueryParamsOrderByItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "name" => Ok(Self::Name),
                "-name" => Ok(Self::Xname),
                "unicodeName" => Ok(Self::UnicodeName),
                "-unicodeName" => Ok(Self::XunicodeName),
                "registrationDate" => Ok(Self::RegistrationDate),
                "-registrationDate" => Ok(Self::XregistrationDate),
                "expirationDate" => Ok(Self::ExpirationDate),
                "-expirationDate" => Ok(Self::XexpirationDate),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for DomainsGetDomainListQueryParamsOrderByItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for DomainsGetDomainListQueryParamsOrderByItem
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for DomainsGetDomainListQueryParamsOrderByItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Domains availability check request
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Domains availability check request",
    ///  "type": "object",
    ///  "required": [
    ///    "domains"
    ///  ],
    ///  "properties": {
    ///    "domains": {
    ///      "description": "List of domain names in ASCII format (A-label)
    /// whose details are to be fetched. The domain name must be provided in a
    /// fully qualified domain format.",
    ///      "examples": [
    ///        [
    ///          "spaceship.dev"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/domainName"
    ///      },
    ///      "maxItems": 20,
    ///      "minItems": 1
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainsGetDomainsAvailabilityRequest {
        ///List of domain names in ASCII format (A-label) whose details are to
        /// be fetched. The domain name must be provided in a fully qualified
        /// domain format.
        pub domains: ::std::vec::Vec<DomainName>,
    }

    ///Domains availability check result
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Domains availability check result",
    ///  "type": "object",
    ///  "required": [
    ///    "domains"
    ///  ],
    ///  "properties": {
    ///    "domains": {
    ///      "readOnly": true,
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/DomainAvailabilityResult"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainsGetDomainsAvailabilityResult {
        pub domains: ::std::vec::Vec<DomainAvailabilityResult>,
    }

    ///Contacts updated successfully. Optionally contacts verification may be
    /// required.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contacts updated successfully. Optionally contacts
    /// verification may be required.",
    ///  "type": "object",
    ///  "required": [
    ///    "verificationStatus"
    ///  ],
    ///  "properties": {
    ///    "verificationStatus": {
    ///      "description": "Status of the RAA verification process. `null` if
    /// RAA procedure is not applied for the domain.",
    ///      "examples": [
    ///        "success"
    ///      ],
    ///      "oneOf": [
    ///        {
    ///          "type": "null"
    ///        },
    ///        {
    ///          "allOf": [
    ///            {
    ///              "$ref":
    /// "#/components/schemas/DomainValidVerificationStatus"
    ///            }
    ///          ]
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct DomainsPutContactsResponse {
        ///Status of the RAA verification process. `null` if RAA procedure is
        /// not applied for the domain.
        #[serde(rename = "verificationStatus")]
        pub verification_status: ::std::option::Option<DomainValidVerificationStatus>,
    }

    ///The dot-separated error code indicating the type of the problem occurred
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The dot-separated error code indicating the type of the
    /// problem occurred",
    ///  "type": "string",
    ///  "maxLength": 255,
    ///  "pattern": "^(business|application|infrastructure)(\\.[a-zA-Z]+)+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ErrorCode(::std::string::String);
    impl ::std::ops::Deref for ErrorCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ErrorCode> for ::std::string::String {
        fn from(value: ErrorCode) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ErrorCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 255usize {
                return Err("longer than 255 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^(business|application|infrastructure)(\\.[a-zA-Z]+)+$")
                        .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^(business|application|infrastructure)(\\.[a-zA-Z]+)+$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ErrorCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ErrorCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ErrorCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ErrorCode {
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

    ///`ErrorDetail`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "detail"
    ///  ],
    ///  "properties": {
    ///    "detail": {
    ///      "description": "A general message about the exception",
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "pattern": "^[\\s|\\S]*$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ErrorDetail {
        ///A general message about the exception
        pub detail: ErrorDetailDetail,
    }

    ///A general message about the exception
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A general message about the exception",
    ///  "readOnly": true,
    ///  "type": "string",
    ///  "pattern": "^[\\s|\\S]*$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ErrorDetailDetail(::std::string::String);
    impl ::std::ops::Deref for ErrorDetailDetail {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ErrorDetailDetail> for ::std::string::String {
        fn from(value: ErrorDetailDetail) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ErrorDetailDetail {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[\\s|\\S]*$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\s|\\S]*$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ErrorDetailDetail {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ErrorDetailDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ErrorDetailDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ErrorDetailDetail {
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

    ///Allowed values for commission percentage split: 0, 25, 50, 75, or 100.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Allowed values for commission percentage split: 0, 25,
    /// 50, 75, or 100.",
    ///  "examples": [
    ///    25
    ///  ],
    ///  "type": "integer",
    ///  "format": "int32",
    ///  "enum": [
    ///    0,
    ///    25,
    ///    50,
    ///    75,
    ///    100
    ///  ],
    ///  "maximum": 100.0,
    ///  "minimum": 0.0
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct FeePercentage(i32);
    impl ::std::ops::Deref for FeePercentage {
        type Target = i32;
        fn deref(&self) -> &i32 {
            &self.0
        }
    }

    impl ::std::convert::From<FeePercentage> for i32 {
        fn from(value: FeePercentage) -> Self {
            value.0
        }
    }

    impl ::std::convert::TryFrom<i32> for FeePercentage {
        type Error = self::error::ConversionError;
        fn try_from(value: i32) -> ::std::result::Result<Self, self::error::ConversionError> {
            if ![0_i32, 25_i32, 50_i32, 75_i32, 100_i32].contains(&value) {
                Err("invalid value".into())
            } else {
                Ok(Self(value))
            }
        }
    }

    impl<'de> ::serde::Deserialize<'de> for FeePercentage {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            Self::try_from(<i32>::deserialize(deserializer)?)
                .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
        }
    }

    ///Commission split between seller and buyer. Determines how the platform
    /// commission is distributed. The seller and buyer percentages must sum to
    /// 100.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Commission split between seller and buyer. Determines
    /// how the platform commission is distributed. The seller and buyer
    /// percentages must sum to 100.",
    ///  "examples": [
    ///    {
    ///      "buyer": 100,
    ///      "seller": 0
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "required": [
    ///    "buyer",
    ///    "seller"
    ///  ],
    ///  "properties": {
    ///    "buyer": {
    ///      "description": "Percentage of the platform commission paid by the
    /// buyer. Allowed values: 0, 25, 50, 75, 100.",
    ///      "examples": [
    ///        25
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/FeePercentage"
    ///        }
    ///      ]
    ///    },
    ///    "seller": {
    ///      "description": "Percentage of the platform commission paid by the
    /// seller. Allowed values: 0, 25, 50, 75, 100.",
    ///      "examples": [
    ///        75
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/FeePercentage"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct FeePercentageShare {
        ///Percentage of the platform commission paid by the buyer. Allowed
        /// values: 0, 25, 50, 75, 100.
        pub buyer: FeePercentage,
        ///Percentage of the platform commission paid by the seller. Allowed
        /// values: 0, 25, 50, 75, 100.
        pub seller: FeePercentage,
    }

    ///`ForbiddenError`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "detail"
    ///  ],
    ///  "properties": {
    ///    "detail": {
    ///      "description": "A general message about the exception",
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "pattern": "^[\\s|\\S]*$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ForbiddenError {
        ///A general message about the exception
        pub detail: ForbiddenErrorDetail,
    }

    ///A general message about the exception
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A general message about the exception",
    ///  "readOnly": true,
    ///  "type": "string",
    ///  "pattern": "^[\\s|\\S]*$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ForbiddenErrorDetail(::std::string::String);
    impl ::std::ops::Deref for ForbiddenErrorDetail {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ForbiddenErrorDetail> for ::std::string::String {
        fn from(value: ForbiddenErrorDetail) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ForbiddenErrorDetail {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[\\s|\\S]*$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\s|\\S]*$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ForbiddenErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ForbiddenErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ForbiddenErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ForbiddenErrorDetail {
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

    ///`Fqdn`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    "ns1.spaceship.dev"
    ///  ],
    ///  "type": "string",
    ///  "format": "hostname",
    ///  "maxLength": 255,
    ///  "minLength": 4
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
    pub struct Fqdn(pub ::std::string::String);
    impl ::std::ops::Deref for Fqdn {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<Fqdn> for ::std::string::String {
        fn from(value: Fqdn) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::string::String> for Fqdn {
        fn from(value: ::std::string::String) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for Fqdn {
        type Err = ::std::convert::Infallible;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::fmt::Display for Fqdn {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///`GetAsyncOperationDetailsOperationId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "maxLength": 36,
    ///  "pattern": "^[a-zA-Z0-9]+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct GetAsyncOperationDetailsOperationId(::std::string::String);
    impl ::std::ops::Deref for GetAsyncOperationDetailsOperationId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<GetAsyncOperationDetailsOperationId> for ::std::string::String {
        fn from(value: GetAsyncOperationDetailsOperationId) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for GetAsyncOperationDetailsOperationId {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 36usize {
                return Err("longer than 36 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[a-zA-Z0-9]+$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[a-zA-Z0-9]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for GetAsyncOperationDetailsOperationId {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetAsyncOperationDetailsOperationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetAsyncOperationDetailsOperationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for GetAsyncOperationDetailsOperationId {
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

    ///`GetDomainListOrderByItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "name",
    ///    "-name",
    ///    "unicodeName",
    ///    "-unicodeName",
    ///    "registrationDate",
    ///    "-registrationDate",
    ///    "expirationDate",
    ///    "-expirationDate"
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
    pub enum GetDomainListOrderByItem {
        #[serde(rename = "name")]
        Name,
        #[serde(rename = "-name")]
        Xname,
        #[serde(rename = "unicodeName")]
        UnicodeName,
        #[serde(rename = "-unicodeName")]
        XunicodeName,
        #[serde(rename = "registrationDate")]
        RegistrationDate,
        #[serde(rename = "-registrationDate")]
        XregistrationDate,
        #[serde(rename = "expirationDate")]
        ExpirationDate,
        #[serde(rename = "-expirationDate")]
        XexpirationDate,
    }

    impl ::std::fmt::Display for GetDomainListOrderByItem {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Name => f.write_str("name"),
                Self::Xname => f.write_str("-name"),
                Self::UnicodeName => f.write_str("unicodeName"),
                Self::XunicodeName => f.write_str("-unicodeName"),
                Self::RegistrationDate => f.write_str("registrationDate"),
                Self::XregistrationDate => f.write_str("-registrationDate"),
                Self::ExpirationDate => f.write_str("expirationDate"),
                Self::XexpirationDate => f.write_str("-expirationDate"),
            }
        }
    }

    impl ::std::str::FromStr for GetDomainListOrderByItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "name" => Ok(Self::Name),
                "-name" => Ok(Self::Xname),
                "unicodeName" => Ok(Self::UnicodeName),
                "-unicodeName" => Ok(Self::XunicodeName),
                "registrationDate" => Ok(Self::RegistrationDate),
                "-registrationDate" => Ok(Self::XregistrationDate),
                "expirationDate" => Ok(Self::ExpirationDate),
                "-expirationDate" => Ok(Self::XexpirationDate),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetDomainListOrderByItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetDomainListOrderByItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetDomainListOrderByItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`GetDomainListResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "items",
    ///    "total"
    ///  ],
    ///  "properties": {
    ///    "items": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/DomainInfo"
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "total": {
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 2147483647.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct GetDomainListResponse {
        pub items: ::std::vec::Vec<DomainInfo>,
        pub total: i32,
    }

    ///`GetResourceRecordsListOrderByItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "type",
    ///    "-type",
    ///    "name",
    ///    "-name"
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
    pub enum GetResourceRecordsListOrderByItem {
        #[serde(rename = "type")]
        Type,
        #[serde(rename = "-type")]
        Xtype,
        #[serde(rename = "name")]
        Name,
        #[serde(rename = "-name")]
        Xname,
    }

    impl ::std::fmt::Display for GetResourceRecordsListOrderByItem {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Type => f.write_str("type"),
                Self::Xtype => f.write_str("-type"),
                Self::Name => f.write_str("name"),
                Self::Xname => f.write_str("-name"),
            }
        }
    }

    impl ::std::str::FromStr for GetResourceRecordsListOrderByItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "type" => Ok(Self::Type),
                "-type" => Ok(Self::Xtype),
                "name" => Ok(Self::Name),
                "-name" => Ok(Self::Xname),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for GetResourceRecordsListOrderByItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for GetResourceRecordsListOrderByItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for GetResourceRecordsListOrderByItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`GetResourceRecordsListResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "items",
    ///    "total"
    ///  ],
    ///  "properties": {
    ///    "items": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/ResourceRecord"
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "total": {
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 2147483647.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct GetResourceRecordsListResponse {
        pub items: ::std::vec::Vec<ResourceRecord>,
        pub total: i32,
    }

    ///`GetSellerHubDomainListResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "items",
    ///    "total"
    ///  ],
    ///  "properties": {
    ///    "items": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/SellerHubDomainResponse"
    ///      },
    ///      "maxItems": 100
    ///    },
    ///    "total": {
    ///      "examples": [
    ///        100
    ///      ],
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 2147483647.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct GetSellerHubDomainListResponse {
        pub items: ::std::vec::Vec<SellerHubDomainResponse>,
        pub total: i32,
    }

    ///`Host`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    "api.www"
    ///  ],
    ///  "type": "string",
    ///  "format": "hostname",
    ///  "maxLength": 255,
    ///  "minLength": 1
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
    pub struct Host(pub ::std::string::String);
    impl ::std::ops::Deref for Host {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<Host> for ::std::string::String {
        fn from(value: Host) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::string::String> for Host {
        fn from(value: ::std::string::String) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for Host {
        type Err = ::std::convert::Infallible;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::fmt::Display for Host {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///`HostNameServers`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "ips"
    ///  ],
    ///  "properties": {
    ///    "ips": {
    ///      "examples": [
    ///        [
    ///          "127.2.2.2",
    ///          "12.22.22.21"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/ipAddress"
    ///      },
    ///      "maxItems": 255
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct HostNameServers {
        pub ips: ::std::vec::Vec<IpAddress>,
    }

    ///`HostNameValue`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    "@"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 253,
    ///  "minLength": 1,
    ///  "pattern":
    /// "^(?!\\.)(@|\\*|([_*]\\.)?(?:(?!-)(?=[^\\.]*[^\\W_])[\\w-]{1,63}(?<!
    /// -)($|\\.)){1,127}(?<!\\.))$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct HostNameValue(::std::string::String);
    impl ::std::ops::Deref for HostNameValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<HostNameValue> for ::std::string::String {
        fn from(value: HostNameValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for HostNameValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 253usize {
                return Err("longer than 253 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> = ::std::sync::LazyLock::new(
                || {
                    :: regress :: Regex :: new ("^(?!\\.)(@|\\*|([_*]\\.)?(?:(?!-)(?=[^\\.]*[^\\W_])[\\w-]{1,63}(?<!-)($|\\.)){1,127}(?<!\\.))$") . unwrap ()
                },
            );
            if PATTERN.find(value).is_none() {
                return Err ("doesn't match pattern \"^(?!\\.)(@|\\*|([_*]\\.)?(?:(?!-)(?=[^\\.]*[^\\W_])[\\w-]{1,63}(?<!-)($|\\.)){1,127}(?<!\\.))$\"" . into ()) ;
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for HostNameValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for HostNameValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for HostNameValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for HostNameValue {
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

    ///Used to deliver configuration information and parameters for how to
    /// access a service via HTTPS.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to deliver configuration information and
    /// parameters for how to access a service via HTTPS.",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "svcParams",
    ///    "svcPriority",
    ///    "targetName",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "port": {
    ///      "description": "Specifies the port number for which the HTTPS
    /// record is applicable.\nIf specified, it must be a single wildcard (an
    /// asterisk symbol) or a string that starts with an underscore and
    /// continues with a number from 1 to 65535.",
    ///      "examples": [
    ///        "_8443"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "*"
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/underscoredPort"
    ///        }
    ///      ]
    ///    },
    ///    "scheme": {
    ///      "description": "Specifies the scheme over which the HTTPS record
    /// applies.\nIt is optional if the port is not specified, otherwise it is
    /// required and must be \"_https\"",
    ///      "examples": [
    ///        "_https"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "_https"
    ///      ]
    ///    },
    ///    "svcParams": {
    ///      "description": "A whitespace-separated list with parameters
    /// describing the alternative endpoint at TargetName (only used in
    /// ServiceMode and otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///      "default": "",
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 0,
    ///      "pattern": ".*"
    ///    },
    ///    "svcPriority": {
    ///      "description": "The priority of this record (relative to others,
    /// with lower values preferred).\nA value of 0 indicates AliasMode and
    /// other values indicate ServiceMode.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "targetName": {
    ///      "description": "A fully qualified domain name (FQDN) or a single
    /// \".\", either the alias target (for AliasMode) or the alternative
    /// endpoint (for ServiceMode).\nFor AliasMode, a TargetName of \".\"
    /// indicates that the service is not available or does not exist.\nFor
    /// ServiceMode, if TargetName has the value \".\", then the owner name of
    /// this record is used as the effective TargetName.",
    ///      "examples": [
    ///        "_443._https.www.example.com"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "."
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/fqdn"
    ///        }
    ///      ],
    ///      "maxLength": 253
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "HTTPS"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct HttpsResourceRecord {
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the port number for which the HTTPS record is applicable.
        ///If specified, it must be a single wildcard (an asterisk symbol) or a
        /// string that starts with an underscore and continues with a number
        /// from 1 to 65535.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub port: ::std::option::Option<HttpsResourceRecordPort>,
        ///Specifies the scheme over which the HTTPS record applies.
        ///It is optional if the port is not specified, otherwise it is
        /// required and must be "_https"
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub scheme: ::std::option::Option<HttpsResourceRecordScheme>,
        ///A whitespace-separated list with parameters describing the
        /// alternative endpoint at TargetName (only used in ServiceMode and
        /// otherwise ignored). Each SvcParam consisting of a
        /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.
        ///
        ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
        /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
        /// "tls-supported-groups".
        ///
        ///Arbitrary keys can be represented using the unknown-key presentation
        /// format "keyNNNNN" where NNNNN is the numeric value of the key type
        /// without leading zeros (Number 0-65535).
        #[serde(rename = "svcParams")]
        pub svc_params: HttpsResourceRecordSvcParams,
        ///The priority of this record (relative to others, with lower values
        /// preferred). A value of 0 indicates AliasMode and other
        /// values indicate ServiceMode.
        #[serde(rename = "svcPriority")]
        pub svc_priority: u16,
        #[serde(rename = "targetName")]
        pub target_name: HttpsResourceRecordTargetName,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: HttpsResourceRecordType,
    }

    ///Used to deliver configuration information and parameters for how to
    /// access a service via HTTPS.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to deliver configuration information and
    /// parameters for how to access a service via HTTPS.",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "svcParams",
    ///    "svcPriority",
    ///    "targetName",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "port": {
    ///      "description": "Specifies the port number for which the HTTPS
    /// record is applicable.\nIf specified, it must be a single wildcard (an
    /// asterisk symbol) or a string that starts with an underscore and
    /// continues with a number from 1 to 65535.",
    ///      "examples": [
    ///        "_8443"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "*"
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/underscoredPort"
    ///        }
    ///      ]
    ///    },
    ///    "scheme": {
    ///      "description": "Specifies the scheme over which the HTTPS record
    /// applies.\nIt is optional if the port is not specified, otherwise it is
    /// required and must be \"_https\"",
    ///      "examples": [
    ///        "_https"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "_https"
    ///      ]
    ///    },
    ///    "svcParams": {
    ///      "description": "A whitespace-separated list with parameters
    /// describing the alternative endpoint at TargetName (only used in
    /// ServiceMode and otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///      "default": "",
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 0,
    ///      "pattern": ".*"
    ///    },
    ///    "svcPriority": {
    ///      "description": "The priority of this record (relative to others,
    /// with lower values preferred).\nA value of 0 indicates AliasMode and
    /// other values indicate ServiceMode.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "targetName": {
    ///      "description": "A fully qualified domain name (FQDN) or a single
    /// \".\", either the alias target (for AliasMode) or the alternative
    /// endpoint (for ServiceMode).\nFor AliasMode, a TargetName of \".\"
    /// indicates that the service is not available or does not exist.\nFor
    /// ServiceMode, if TargetName has the value \".\", then the owner name of
    /// this record is used as the effective TargetName.",
    ///      "examples": [
    ///        "_443._https.www.example.com"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "."
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/fqdn"
    ///        }
    ///      ],
    ///      "maxLength": 253
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "HTTPS"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct HttpsResourceRecordCreateOrUpdateItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the port number for which the HTTPS record is applicable.
        ///If specified, it must be a single wildcard (an asterisk symbol) or a
        /// string that starts with an underscore and continues with a number
        /// from 1 to 65535.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub port: ::std::option::Option<HttpsResourceRecordCreateOrUpdateItemPort>,
        ///Specifies the scheme over which the HTTPS record applies.
        ///It is optional if the port is not specified, otherwise it is
        /// required and must be "_https"
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub scheme: ::std::option::Option<HttpsResourceRecordCreateOrUpdateItemScheme>,
        ///A whitespace-separated list with parameters describing the
        /// alternative endpoint at TargetName (only used in ServiceMode and
        /// otherwise ignored). Each SvcParam consisting of a
        /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.
        ///
        ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
        /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
        /// "tls-supported-groups".
        ///
        ///Arbitrary keys can be represented using the unknown-key presentation
        /// format "keyNNNNN" where NNNNN is the numeric value of the key type
        /// without leading zeros (Number 0-65535).
        #[serde(rename = "svcParams")]
        pub svc_params: HttpsResourceRecordCreateOrUpdateItemSvcParams,
        ///The priority of this record (relative to others, with lower values
        /// preferred). A value of 0 indicates AliasMode and other
        /// values indicate ServiceMode.
        #[serde(rename = "svcPriority")]
        pub svc_priority: u16,
        #[serde(rename = "targetName")]
        pub target_name: HttpsResourceRecordCreateOrUpdateItemTargetName,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: HttpsResourceRecordCreateOrUpdateItemType,
    }

    ///Specifies the port number for which the HTTPS record is applicable.
    ///If specified, it must be a single wildcard (an asterisk symbol) or a
    /// string that starts with an underscore and continues with a number from 1
    /// to 65535.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the port number for which the HTTPS record is
    /// applicable.\nIf specified, it must be a single wildcard (an asterisk
    /// symbol) or a string that starts with an underscore and continues with a
    /// number from 1 to 65535.",
    ///  "examples": [
    ///    "_8443"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "*"
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/underscoredPort"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct HttpsResourceRecordCreateOrUpdateItemPort {
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_0: ::std::option::Option<HttpsResourceRecordCreateOrUpdateItemPortSubtype0>,
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_1: ::std::option::Option<UnderscoredPort>,
    }

    impl ::std::default::Default for HttpsResourceRecordCreateOrUpdateItemPort {
        fn default() -> Self {
            Self {
                subtype_0: Default::default(),
                subtype_1: Default::default(),
            }
        }
    }

    ///`HttpsResourceRecordCreateOrUpdateItemPortSubtype0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "*"
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
    pub enum HttpsResourceRecordCreateOrUpdateItemPortSubtype0 {
        #[serde(rename = "*")]
        X,
    }

    impl ::std::fmt::Display for HttpsResourceRecordCreateOrUpdateItemPortSubtype0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("*"),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordCreateOrUpdateItemPortSubtype0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "*" => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordCreateOrUpdateItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for HttpsResourceRecordCreateOrUpdateItemPortSubtype0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for HttpsResourceRecordCreateOrUpdateItemPortSubtype0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Specifies the scheme over which the HTTPS record applies.
    ///It is optional if the port is not specified, otherwise it is required
    /// and must be "_https"
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the scheme over which the HTTPS record
    /// applies.\nIt is optional if the port is not specified, otherwise it is
    /// required and must be \"_https\"",
    ///  "examples": [
    ///    "_https"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "_https"
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
    pub enum HttpsResourceRecordCreateOrUpdateItemScheme {
        #[serde(rename = "_https")]
        Https,
    }

    impl ::std::fmt::Display for HttpsResourceRecordCreateOrUpdateItemScheme {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Https => f.write_str("_https"),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordCreateOrUpdateItemScheme {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "_https" => Ok(Self::Https),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordCreateOrUpdateItemScheme {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for HttpsResourceRecordCreateOrUpdateItemScheme
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for HttpsResourceRecordCreateOrUpdateItemScheme
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///A whitespace-separated list with parameters describing the alternative
    /// endpoint at TargetName (only used in ServiceMode and otherwise ignored).
    /// Each SvcParam consisting of a SvcParamKey=SvcParamValue pair or a
    /// standalone SvcParamKey.
    ///
    ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
    /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
    /// "tls-supported-groups".
    ///
    ///Arbitrary keys can be represented using the unknown-key presentation
    /// format "keyNNNNN" where NNNNN is the numeric value of the key type
    /// without leading zeros (Number 0-65535).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A whitespace-separated list with parameters describing
    /// the alternative endpoint at TargetName (only used in ServiceMode and
    /// otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///  "default": "",
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 0,
    ///  "pattern": ".*"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct HttpsResourceRecordCreateOrUpdateItemSvcParams(::std::string::String);
    impl ::std::ops::Deref for HttpsResourceRecordCreateOrUpdateItemSvcParams {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<HttpsResourceRecordCreateOrUpdateItemSvcParams>
        for ::std::string::String
    {
        fn from(value: HttpsResourceRecordCreateOrUpdateItemSvcParams) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for HttpsResourceRecordCreateOrUpdateItemSvcParams {
        fn default() -> Self {
            HttpsResourceRecordCreateOrUpdateItemSvcParams("".to_string())
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordCreateOrUpdateItemSvcParams {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 0usize {
                return Err("shorter than 0 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new(".*").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \".*\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordCreateOrUpdateItemSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for HttpsResourceRecordCreateOrUpdateItemSvcParams
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for HttpsResourceRecordCreateOrUpdateItemSvcParams
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for HttpsResourceRecordCreateOrUpdateItemSvcParams {
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

    ///A fully qualified domain name (FQDN) or a single ".", either the alias
    /// target (for AliasMode) or the alternative endpoint (for ServiceMode).
    /// For AliasMode, a TargetName of "." indicates that the service is not
    /// available or does not exist. For ServiceMode, if TargetName has the
    /// value ".", then the owner name of this record is used as the effective
    /// TargetName.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A fully qualified domain name (FQDN) or a single \".\",
    /// either the alias target (for AliasMode) or the alternative endpoint (for
    /// ServiceMode).\nFor AliasMode, a TargetName of \".\" indicates that the
    /// service is not available or does not exist.\nFor ServiceMode, if
    /// TargetName has the value \".\", then the owner name of this record is
    /// used as the effective TargetName.",
    ///  "examples": [
    ///    "_443._https.www.example.com"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/fqdn"
    ///    }
    ///  ],
    ///  "maxLength": 253
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum HttpsResourceRecordCreateOrUpdateItemTargetName {
        Variant0(HttpsResourceRecordCreateOrUpdateItemTargetNameVariant0),
        Variant1(::std::string::String),
    }

    impl ::std::fmt::Display for HttpsResourceRecordCreateOrUpdateItemTargetName {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                Self::Variant0(x) => x.fmt(f),
                Self::Variant1(x) => x.fmt(f),
            }
        }
    }

    impl ::std::convert::From<HttpsResourceRecordCreateOrUpdateItemTargetNameVariant0>
        for HttpsResourceRecordCreateOrUpdateItemTargetName
    {
        fn from(value: HttpsResourceRecordCreateOrUpdateItemTargetNameVariant0) -> Self {
            Self::Variant0(value)
        }
    }

    ///`HttpsResourceRecordCreateOrUpdateItemTargetNameVariant0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "maxLength": 253
    ///    },
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "not": {
    ///        "$ref": "#/components/schemas/fqdn"
    ///      }
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
    pub enum HttpsResourceRecordCreateOrUpdateItemTargetNameVariant0 {
        #[serde(rename = ".")]
        X,
    }

    impl ::std::fmt::Display for HttpsResourceRecordCreateOrUpdateItemTargetNameVariant0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("."),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordCreateOrUpdateItemTargetNameVariant0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "." => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordCreateOrUpdateItemTargetNameVariant0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for HttpsResourceRecordCreateOrUpdateItemTargetNameVariant0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for HttpsResourceRecordCreateOrUpdateItemTargetNameVariant0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`HttpsResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "HTTPS"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum HttpsResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "HTTPS")]
        Https,
    }

    impl ::std::fmt::Display for HttpsResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Https => f.write_str("HTTPS"),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "HTTPS" => Ok(Self::Https),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for HttpsResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for HttpsResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to deliver configuration information and parameters for how to
    /// access a service via HTTPS.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to deliver configuration information and
    /// parameters for how to access a service via HTTPS.",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "svcParams",
    ///    "svcPriority",
    ///    "targetName",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "port": {
    ///      "description": "Specifies the port number for which the HTTPS
    /// record is applicable.\nIf specified, it must be a single wildcard (an
    /// asterisk symbol) or a string that starts with an underscore and
    /// continues with a number from 1 to 65535.",
    ///      "examples": [
    ///        "_8443"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "*"
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/underscoredPort"
    ///        }
    ///      ]
    ///    },
    ///    "scheme": {
    ///      "description": "Specifies the scheme over which the HTTPS record
    /// applies.\nIt is optional if the port is not specified, otherwise it is
    /// required and must be \"_https\"",
    ///      "examples": [
    ///        "_https"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "_https"
    ///      ]
    ///    },
    ///    "svcParams": {
    ///      "description": "A whitespace-separated list with parameters
    /// describing the alternative endpoint at TargetName (only used in
    /// ServiceMode and otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///      "default": "",
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 0,
    ///      "pattern": ".*"
    ///    },
    ///    "svcPriority": {
    ///      "description": "The priority of this record (relative to others,
    /// with lower values preferred).\nA value of 0 indicates AliasMode and
    /// other values indicate ServiceMode.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "targetName": {
    ///      "description": "A fully qualified domain name (FQDN) or a single
    /// \".\", either the alias target (for AliasMode) or the alternative
    /// endpoint (for ServiceMode).\nFor AliasMode, a TargetName of \".\"
    /// indicates that the service is not available or does not exist.\nFor
    /// ServiceMode, if TargetName has the value \".\", then the owner name of
    /// this record is used as the effective TargetName.",
    ///      "examples": [
    ///        "_443._https.www.example.com"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "."
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/fqdn"
    ///        }
    ///      ],
    ///      "maxLength": 253
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "HTTPS"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct HttpsResourceRecordDeleteItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the port number for which the HTTPS record is applicable.
        ///If specified, it must be a single wildcard (an asterisk symbol) or a
        /// string that starts with an underscore and continues with a number
        /// from 1 to 65535.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub port: ::std::option::Option<HttpsResourceRecordDeleteItemPort>,
        ///Specifies the scheme over which the HTTPS record applies.
        ///It is optional if the port is not specified, otherwise it is
        /// required and must be "_https"
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub scheme: ::std::option::Option<HttpsResourceRecordDeleteItemScheme>,
        ///A whitespace-separated list with parameters describing the
        /// alternative endpoint at TargetName (only used in ServiceMode and
        /// otherwise ignored). Each SvcParam consisting of a
        /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.
        ///
        ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
        /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
        /// "tls-supported-groups".
        ///
        ///Arbitrary keys can be represented using the unknown-key presentation
        /// format "keyNNNNN" where NNNNN is the numeric value of the key type
        /// without leading zeros (Number 0-65535).
        #[serde(rename = "svcParams")]
        pub svc_params: HttpsResourceRecordDeleteItemSvcParams,
        ///The priority of this record (relative to others, with lower values
        /// preferred). A value of 0 indicates AliasMode and other
        /// values indicate ServiceMode.
        #[serde(rename = "svcPriority")]
        pub svc_priority: u16,
        #[serde(rename = "targetName")]
        pub target_name: HttpsResourceRecordDeleteItemTargetName,
        #[serde(rename = "type")]
        pub type_: HttpsResourceRecordDeleteItemType,
    }

    ///Specifies the port number for which the HTTPS record is applicable.
    ///If specified, it must be a single wildcard (an asterisk symbol) or a
    /// string that starts with an underscore and continues with a number from 1
    /// to 65535.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the port number for which the HTTPS record is
    /// applicable.\nIf specified, it must be a single wildcard (an asterisk
    /// symbol) or a string that starts with an underscore and continues with a
    /// number from 1 to 65535.",
    ///  "examples": [
    ///    "_8443"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "*"
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/underscoredPort"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct HttpsResourceRecordDeleteItemPort {
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_0: ::std::option::Option<HttpsResourceRecordDeleteItemPortSubtype0>,
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_1: ::std::option::Option<UnderscoredPort>,
    }

    impl ::std::default::Default for HttpsResourceRecordDeleteItemPort {
        fn default() -> Self {
            Self {
                subtype_0: Default::default(),
                subtype_1: Default::default(),
            }
        }
    }

    ///`HttpsResourceRecordDeleteItemPortSubtype0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "*"
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
    pub enum HttpsResourceRecordDeleteItemPortSubtype0 {
        #[serde(rename = "*")]
        X,
    }

    impl ::std::fmt::Display for HttpsResourceRecordDeleteItemPortSubtype0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("*"),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordDeleteItemPortSubtype0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "*" => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordDeleteItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for HttpsResourceRecordDeleteItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for HttpsResourceRecordDeleteItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Specifies the scheme over which the HTTPS record applies.
    ///It is optional if the port is not specified, otherwise it is required
    /// and must be "_https"
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the scheme over which the HTTPS record
    /// applies.\nIt is optional if the port is not specified, otherwise it is
    /// required and must be \"_https\"",
    ///  "examples": [
    ///    "_https"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "_https"
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
    pub enum HttpsResourceRecordDeleteItemScheme {
        #[serde(rename = "_https")]
        Https,
    }

    impl ::std::fmt::Display for HttpsResourceRecordDeleteItemScheme {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Https => f.write_str("_https"),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordDeleteItemScheme {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "_https" => Ok(Self::Https),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordDeleteItemScheme {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for HttpsResourceRecordDeleteItemScheme {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for HttpsResourceRecordDeleteItemScheme {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///A whitespace-separated list with parameters describing the alternative
    /// endpoint at TargetName (only used in ServiceMode and otherwise ignored).
    /// Each SvcParam consisting of a SvcParamKey=SvcParamValue pair or a
    /// standalone SvcParamKey.
    ///
    ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
    /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
    /// "tls-supported-groups".
    ///
    ///Arbitrary keys can be represented using the unknown-key presentation
    /// format "keyNNNNN" where NNNNN is the numeric value of the key type
    /// without leading zeros (Number 0-65535).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A whitespace-separated list with parameters describing
    /// the alternative endpoint at TargetName (only used in ServiceMode and
    /// otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///  "default": "",
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 0,
    ///  "pattern": ".*"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct HttpsResourceRecordDeleteItemSvcParams(::std::string::String);
    impl ::std::ops::Deref for HttpsResourceRecordDeleteItemSvcParams {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<HttpsResourceRecordDeleteItemSvcParams> for ::std::string::String {
        fn from(value: HttpsResourceRecordDeleteItemSvcParams) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for HttpsResourceRecordDeleteItemSvcParams {
        fn default() -> Self {
            HttpsResourceRecordDeleteItemSvcParams("".to_string())
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordDeleteItemSvcParams {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 0usize {
                return Err("shorter than 0 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new(".*").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \".*\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordDeleteItemSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for HttpsResourceRecordDeleteItemSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for HttpsResourceRecordDeleteItemSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for HttpsResourceRecordDeleteItemSvcParams {
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

    ///A fully qualified domain name (FQDN) or a single ".", either the alias
    /// target (for AliasMode) or the alternative endpoint (for ServiceMode).
    /// For AliasMode, a TargetName of "." indicates that the service is not
    /// available or does not exist. For ServiceMode, if TargetName has the
    /// value ".", then the owner name of this record is used as the effective
    /// TargetName.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A fully qualified domain name (FQDN) or a single \".\",
    /// either the alias target (for AliasMode) or the alternative endpoint (for
    /// ServiceMode).\nFor AliasMode, a TargetName of \".\" indicates that the
    /// service is not available or does not exist.\nFor ServiceMode, if
    /// TargetName has the value \".\", then the owner name of this record is
    /// used as the effective TargetName.",
    ///  "examples": [
    ///    "_443._https.www.example.com"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/fqdn"
    ///    }
    ///  ],
    ///  "maxLength": 253
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum HttpsResourceRecordDeleteItemTargetName {
        Variant0(HttpsResourceRecordDeleteItemTargetNameVariant0),
        Variant1(::std::string::String),
    }

    impl ::std::fmt::Display for HttpsResourceRecordDeleteItemTargetName {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                Self::Variant0(x) => x.fmt(f),
                Self::Variant1(x) => x.fmt(f),
            }
        }
    }

    impl ::std::convert::From<HttpsResourceRecordDeleteItemTargetNameVariant0>
        for HttpsResourceRecordDeleteItemTargetName
    {
        fn from(value: HttpsResourceRecordDeleteItemTargetNameVariant0) -> Self {
            Self::Variant0(value)
        }
    }

    ///`HttpsResourceRecordDeleteItemTargetNameVariant0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "maxLength": 253
    ///    },
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "not": {
    ///        "$ref": "#/components/schemas/fqdn"
    ///      }
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
    pub enum HttpsResourceRecordDeleteItemTargetNameVariant0 {
        #[serde(rename = ".")]
        X,
    }

    impl ::std::fmt::Display for HttpsResourceRecordDeleteItemTargetNameVariant0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("."),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordDeleteItemTargetNameVariant0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "." => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordDeleteItemTargetNameVariant0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for HttpsResourceRecordDeleteItemTargetNameVariant0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for HttpsResourceRecordDeleteItemTargetNameVariant0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`HttpsResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "HTTPS"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum HttpsResourceRecordDeleteItemType {
        #[serde(rename = "HTTPS")]
        Https,
    }

    impl ::std::fmt::Display for HttpsResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Https => f.write_str("HTTPS"),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "HTTPS" => Ok(Self::Https),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for HttpsResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for HttpsResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Specifies the port number for which the HTTPS record is applicable.
    ///If specified, it must be a single wildcard (an asterisk symbol) or a
    /// string that starts with an underscore and continues with a number from 1
    /// to 65535.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the port number for which the HTTPS record is
    /// applicable.\nIf specified, it must be a single wildcard (an asterisk
    /// symbol) or a string that starts with an underscore and continues with a
    /// number from 1 to 65535.",
    ///  "examples": [
    ///    "_8443"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "*"
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/underscoredPort"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct HttpsResourceRecordPort {
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_0: ::std::option::Option<HttpsResourceRecordPortSubtype0>,
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_1: ::std::option::Option<UnderscoredPort>,
    }

    impl ::std::default::Default for HttpsResourceRecordPort {
        fn default() -> Self {
            Self {
                subtype_0: Default::default(),
                subtype_1: Default::default(),
            }
        }
    }

    ///`HttpsResourceRecordPortSubtype0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "*"
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
    pub enum HttpsResourceRecordPortSubtype0 {
        #[serde(rename = "*")]
        X,
    }

    impl ::std::fmt::Display for HttpsResourceRecordPortSubtype0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("*"),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordPortSubtype0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "*" => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for HttpsResourceRecordPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for HttpsResourceRecordPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Specifies the scheme over which the HTTPS record applies.
    ///It is optional if the port is not specified, otherwise it is required
    /// and must be "_https"
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the scheme over which the HTTPS record
    /// applies.\nIt is optional if the port is not specified, otherwise it is
    /// required and must be \"_https\"",
    ///  "examples": [
    ///    "_https"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "_https"
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
    pub enum HttpsResourceRecordScheme {
        #[serde(rename = "_https")]
        Https,
    }

    impl ::std::fmt::Display for HttpsResourceRecordScheme {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Https => f.write_str("_https"),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordScheme {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "_https" => Ok(Self::Https),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordScheme {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for HttpsResourceRecordScheme {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for HttpsResourceRecordScheme {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///A whitespace-separated list with parameters describing the alternative
    /// endpoint at TargetName (only used in ServiceMode and otherwise ignored).
    /// Each SvcParam consisting of a SvcParamKey=SvcParamValue pair or a
    /// standalone SvcParamKey.
    ///
    ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
    /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
    /// "tls-supported-groups".
    ///
    ///Arbitrary keys can be represented using the unknown-key presentation
    /// format "keyNNNNN" where NNNNN is the numeric value of the key type
    /// without leading zeros (Number 0-65535).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A whitespace-separated list with parameters describing
    /// the alternative endpoint at TargetName (only used in ServiceMode and
    /// otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///  "default": "",
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 0,
    ///  "pattern": ".*"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct HttpsResourceRecordSvcParams(::std::string::String);
    impl ::std::ops::Deref for HttpsResourceRecordSvcParams {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<HttpsResourceRecordSvcParams> for ::std::string::String {
        fn from(value: HttpsResourceRecordSvcParams) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for HttpsResourceRecordSvcParams {
        fn default() -> Self {
            HttpsResourceRecordSvcParams("".to_string())
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordSvcParams {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 0usize {
                return Err("shorter than 0 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new(".*").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \".*\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for HttpsResourceRecordSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for HttpsResourceRecordSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for HttpsResourceRecordSvcParams {
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

    ///A fully qualified domain name (FQDN) or a single ".", either the alias
    /// target (for AliasMode) or the alternative endpoint (for ServiceMode).
    /// For AliasMode, a TargetName of "." indicates that the service is not
    /// available or does not exist. For ServiceMode, if TargetName has the
    /// value ".", then the owner name of this record is used as the effective
    /// TargetName.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A fully qualified domain name (FQDN) or a single \".\",
    /// either the alias target (for AliasMode) or the alternative endpoint (for
    /// ServiceMode).\nFor AliasMode, a TargetName of \".\" indicates that the
    /// service is not available or does not exist.\nFor ServiceMode, if
    /// TargetName has the value \".\", then the owner name of this record is
    /// used as the effective TargetName.",
    ///  "examples": [
    ///    "_443._https.www.example.com"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/fqdn"
    ///    }
    ///  ],
    ///  "maxLength": 253
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum HttpsResourceRecordTargetName {
        Variant0(HttpsResourceRecordTargetNameVariant0),
        Variant1(::std::string::String),
    }

    impl ::std::fmt::Display for HttpsResourceRecordTargetName {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                Self::Variant0(x) => x.fmt(f),
                Self::Variant1(x) => x.fmt(f),
            }
        }
    }

    impl ::std::convert::From<HttpsResourceRecordTargetNameVariant0> for HttpsResourceRecordTargetName {
        fn from(value: HttpsResourceRecordTargetNameVariant0) -> Self {
            Self::Variant0(value)
        }
    }

    ///`HttpsResourceRecordTargetNameVariant0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "maxLength": 253
    ///    },
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "not": {
    ///        "$ref": "#/components/schemas/fqdn"
    ///      }
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
    pub enum HttpsResourceRecordTargetNameVariant0 {
        #[serde(rename = ".")]
        X,
    }

    impl ::std::fmt::Display for HttpsResourceRecordTargetNameVariant0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("."),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordTargetNameVariant0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "." => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordTargetNameVariant0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for HttpsResourceRecordTargetNameVariant0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for HttpsResourceRecordTargetNameVariant0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`HttpsResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "HTTPS"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum HttpsResourceRecordType {
        #[serde(rename = "HTTPS")]
        Https,
    }

    impl ::std::fmt::Display for HttpsResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Https => f.write_str("HTTPS"),
            }
        }
    }

    impl ::std::str::FromStr for HttpsResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "HTTPS" => Ok(Self::Https),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for HttpsResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for HttpsResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for HttpsResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///An IP address represented as a string, which can be either IPv4 or IPv6
    /// format.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "An IP address represented as a string, which can be
    /// either IPv4 or IPv6 format.",
    ///  "type": "string",
    ///  "format": "ip",
    ///  "maxLength": 39
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct IpAddress(pub ::std::net::IpAddr);
    impl ::std::ops::Deref for IpAddress {
        type Target = ::std::net::IpAddr;
        fn deref(&self) -> &::std::net::IpAddr {
            &self.0
        }
    }

    impl ::std::convert::From<IpAddress> for ::std::net::IpAddr {
        fn from(value: IpAddress) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::net::IpAddr> for IpAddress {
        fn from(value: ::std::net::IpAddr) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for IpAddress {
        type Err = <::std::net::IpAddr as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for IpAddress {
        type Error = <::std::net::IpAddr as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for IpAddress {
        type Error = <::std::net::IpAddr as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for IpAddress {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///IPv4 address
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "IPv4 address",
    ///  "examples": [
    ///    "127.0.0.1"
    ///  ],
    ///  "type": "string",
    ///  "format": "ipv4",
    ///  "maxLength": 15
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct IpV4Address(pub ::std::net::Ipv4Addr);
    impl ::std::ops::Deref for IpV4Address {
        type Target = ::std::net::Ipv4Addr;
        fn deref(&self) -> &::std::net::Ipv4Addr {
            &self.0
        }
    }

    impl ::std::convert::From<IpV4Address> for ::std::net::Ipv4Addr {
        fn from(value: IpV4Address) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::net::Ipv4Addr> for IpV4Address {
        fn from(value: ::std::net::Ipv4Addr) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for IpV4Address {
        type Err = <::std::net::Ipv4Addr as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for IpV4Address {
        type Error = <::std::net::Ipv4Addr as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for IpV4Address {
        type Error = <::std::net::Ipv4Addr as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for IpV4Address {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///IPv6 address
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "IPv6 address",
    ///  "examples": [
    ///    "2001:0db8:85a3:0000:0000:8a2e:0370:7334"
    ///  ],
    ///  "type": "string",
    ///  "format": "ipv6",
    ///  "maxLength": 39
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct IpV6Address(pub ::std::net::Ipv6Addr);
    impl ::std::ops::Deref for IpV6Address {
        type Target = ::std::net::Ipv6Addr;
        fn deref(&self) -> &::std::net::Ipv6Addr {
            &self.0
        }
    }

    impl ::std::convert::From<IpV6Address> for ::std::net::Ipv6Addr {
        fn from(value: IpV6Address) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::net::Ipv6Addr> for IpV6Address {
        fn from(value: ::std::net::Ipv6Addr) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for IpV6Address {
        type Err = <::std::net::Ipv6Addr as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for IpV6Address {
        type Error = <::std::net::Ipv6Addr as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for IpV6Address {
        type Error = <::std::net::Ipv6Addr as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for IpV6Address {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///ISO-8601 DateTime
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "ISO-8601 DateTime",
    ///  "examples": [
    ///    "2100-01-01T00:00:00.000Z"
    ///  ],
    ///  "type": "string",
    ///  "format": "date-time",
    ///  "maxLength": 27
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct IsoDate(pub ::chrono::DateTime<::chrono::offset::Utc>);
    impl ::std::ops::Deref for IsoDate {
        type Target = ::chrono::DateTime<::chrono::offset::Utc>;
        fn deref(&self) -> &::chrono::DateTime<::chrono::offset::Utc> {
            &self.0
        }
    }

    impl ::std::convert::From<IsoDate> for ::chrono::DateTime<::chrono::offset::Utc> {
        fn from(value: IsoDate) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::chrono::DateTime<::chrono::offset::Utc>> for IsoDate {
        fn from(value: ::chrono::DateTime<::chrono::offset::Utc>) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for IsoDate {
        type Err = <::chrono::DateTime<::chrono::offset::Utc> as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for IsoDate {
        type Error = <::chrono::DateTime<::chrono::offset::Utc> as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for IsoDate {
        type Error = <::chrono::DateTime<::chrono::offset::Utc> as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for IsoDate {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Specifies the mail servers responsible for receiving email messages on
    /// behalf of a domain
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the mail servers responsible for receiving
    /// email messages on behalf of a domain",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "exchange",
    ///    "preference",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "exchange": {
    ///      "description": "Mail server that accepts mail",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "preference": {
    ///      "description": "Preference (distance) number of mail server",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "MX"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct MxResourceRecord {
        ///Mail server that accepts mail
        pub exchange: HostNameValue,
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Preference (distance) number of mail server
        pub preference: u16,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: MxResourceRecordType,
    }

    ///Specifies the mail servers responsible for receiving email messages on
    /// behalf of a domain
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the mail servers responsible for receiving
    /// email messages on behalf of a domain",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "exchange",
    ///    "preference",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "exchange": {
    ///      "description": "Mail server that accepts mail",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "preference": {
    ///      "description": "Preference (distance) number of mail server",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "MX"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct MxResourceRecordCreateOrUpdateItem {
        ///Mail server that accepts mail
        pub exchange: HostNameValue,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Preference (distance) number of mail server
        pub preference: u16,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: MxResourceRecordCreateOrUpdateItemType,
    }

    ///`MxResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "MX"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum MxResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "MX")]
        Mx,
    }

    impl ::std::fmt::Display for MxResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Mx => f.write_str("MX"),
            }
        }
    }

    impl ::std::str::FromStr for MxResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "MX" => Ok(Self::Mx),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for MxResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for MxResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for MxResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Specifies the mail servers responsible for receiving email messages on
    /// behalf of a domain
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the mail servers responsible for receiving
    /// email messages on behalf of a domain",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "exchange",
    ///    "preference",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "exchange": {
    ///      "description": "Mail server that accepts mail",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "preference": {
    ///      "description": "Preference (distance) number of mail server",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "MX"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct MxResourceRecordDeleteItem {
        ///Mail server that accepts mail
        pub exchange: HostNameValue,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Preference (distance) number of mail server
        pub preference: u16,
        #[serde(rename = "type")]
        pub type_: MxResourceRecordDeleteItemType,
    }

    ///`MxResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "MX"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum MxResourceRecordDeleteItemType {
        #[serde(rename = "MX")]
        Mx,
    }

    impl ::std::fmt::Display for MxResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Mx => f.write_str("MX"),
            }
        }
    }

    impl ::std::str::FromStr for MxResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "MX" => Ok(Self::Mx),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for MxResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for MxResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for MxResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`MxResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "MX"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum MxResourceRecordType {
        #[serde(rename = "MX")]
        Mx,
    }

    impl ::std::fmt::Display for MxResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Mx => f.write_str("MX"),
            }
        }
    }

    impl ::std::str::FromStr for MxResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "MX" => Ok(Self::Mx),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for MxResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for MxResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for MxResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Specifies which name servers are authoritative for a specific domain
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies which name servers are authoritative for a
    /// specific domain",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "nameserver",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "nameserver": {
    ///      "description": "Nameserver name",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "NS"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct NsResourceRecord {
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Nameserver name
        pub nameserver: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: NsResourceRecordType,
    }

    ///Specifies which name servers are authoritative for a specific domain
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies which name servers are authoritative for a
    /// specific domain",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "nameserver",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "nameserver": {
    ///      "description": "Nameserver name",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "NS"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct NsResourceRecordCreateOrUpdateItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Nameserver name
        pub nameserver: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: NsResourceRecordCreateOrUpdateItemType,
    }

    ///`NsResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "NS"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum NsResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "NS")]
        Ns,
    }

    impl ::std::fmt::Display for NsResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Ns => f.write_str("NS"),
            }
        }
    }

    impl ::std::str::FromStr for NsResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "NS" => Ok(Self::Ns),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for NsResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for NsResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for NsResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Specifies which name servers are authoritative for a specific domain
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies which name servers are authoritative for a
    /// specific domain",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "nameserver",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "nameserver": {
    ///      "description": "Nameserver name",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "NS"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct NsResourceRecordDeleteItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Nameserver name
        pub nameserver: HostNameValue,
        #[serde(rename = "type")]
        pub type_: NsResourceRecordDeleteItemType,
    }

    ///`NsResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "NS"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum NsResourceRecordDeleteItemType {
        #[serde(rename = "NS")]
        Ns,
    }

    impl ::std::fmt::Display for NsResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Ns => f.write_str("NS"),
            }
        }
    }

    impl ::std::str::FromStr for NsResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "NS" => Ok(Self::Ns),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for NsResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for NsResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for NsResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`NsResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "NS"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum NsResourceRecordType {
        #[serde(rename = "NS")]
        Ns,
    }

    impl ::std::fmt::Display for NsResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Ns => f.write_str("NS"),
            }
        }
    }

    impl ::std::str::FromStr for NsResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "NS" => Ok(Self::Ns),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for NsResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for NsResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for NsResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`ObjectNotFoundError`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "detail"
    ///  ],
    ///  "properties": {
    ///    "detail": {
    ///      "description": "A general message about the exception",
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "pattern": "^[\\s|\\S]*$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ObjectNotFoundError {
        ///A general message about the exception
        pub detail: ObjectNotFoundErrorDetail,
    }

    ///A general message about the exception
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A general message about the exception",
    ///  "readOnly": true,
    ///  "type": "string",
    ///  "pattern": "^[\\s|\\S]*$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ObjectNotFoundErrorDetail(::std::string::String);
    impl ::std::ops::Deref for ObjectNotFoundErrorDetail {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ObjectNotFoundErrorDetail> for ::std::string::String {
        fn from(value: ObjectNotFoundErrorDetail) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ObjectNotFoundErrorDetail {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[\\s|\\S]*$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\s|\\S]*$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ObjectNotFoundErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ObjectNotFoundErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ObjectNotFoundErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ObjectNotFoundErrorDetail {
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

    ///16-characters unique identifier of the operation
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "16-characters unique identifier of the operation",
    ///  "type": "string",
    ///  "maxLength": 16,
    ///  "pattern": "^[a-z0-9]{16}$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct OperationId(::std::string::String);
    impl ::std::ops::Deref for OperationId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<OperationId> for ::std::string::String {
        fn from(value: OperationId) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for OperationId {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 16usize {
                return Err("longer than 16 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[a-z0-9]{16}$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[a-z0-9]{16}$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for OperationId {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for OperationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for OperationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for OperationId {
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

    ///`PersonalNameserverRecord`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "host",
    ///    "ips"
    ///  ],
    ///  "properties": {
    ///    "host": {
    ///      "description": "The host name of the personal nameserver",
    ///      "examples": [
    ///        "ns1"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/host"
    ///        }
    ///      ]
    ///    },
    ///    "ips": {
    ///      "description": "List of IP addresses associated with the personal
    /// nameserver host",
    ///      "examples": [
    ///        [
    ///          "127.2.2.2",
    ///          "12.22.22.21"
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/ipAddress"
    ///      },
    ///      "maxItems": 16,
    ///      "minItems": 1
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PersonalNameserverRecord {
        ///The host name of the personal nameserver
        pub host: Host,
        ///List of IP addresses associated with the personal nameserver host
        pub ips: ::std::vec::Vec<IpAddress>,
    }

    ///`PersonalNameservers`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "records"
    ///  ],
    ///  "properties": {
    ///    "records": {
    ///      "description": "List of hosts and IP addresses associated with the
    /// personal nameservers",
    ///      "examples": [
    ///        [
    ///          {
    ///            "host": "ns1",
    ///            "ips": [
    ///              "127.0.0.1",
    ///              "127.0.0.2"
    ///            ]
    ///          },
    ///          {
    ///            "host": "ns2",
    ///            "ips": [
    ///              "2001:0db8:85a3:0000:0000:8a2e:0370:7334"
    ///            ]
    ///          }
    ///        ]
    ///      ],
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/PersonalNameserverRecord"
    ///      },
    ///      "maxItems": 255,
    ///      "minItems": 1
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PersonalNameservers {
        ///List of hosts and IP addresses associated with the personal
        /// nameservers
        pub records: ::std::vec::Vec<PersonalNameserverRecord>,
    }

    ///Phone number
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Phone number",
    ///  "examples": [
    ///    "+1.123456789"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 17,
    ///  "minLength": 7,
    ///  "pattern": "^\\+\\d{1,3}\\.\\d{4,}$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct Phone(::std::string::String);
    impl ::std::ops::Deref for Phone {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<Phone> for ::std::string::String {
        fn from(value: Phone) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for Phone {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 17usize {
                return Err("longer than 17 characters".into());
            }
            if value.chars().count() < 7usize {
                return Err("shorter than 7 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^\\+\\d{1,3}\\.\\d{4,}$").unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^\\+\\d{1,3}\\.\\d{4,}$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for Phone {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for Phone {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for Phone {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for Phone {
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

    ///Phone number extension
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Phone number extension",
    ///  "examples": [
    ///    "256"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 7,
    ///  "pattern": "^\\d{0,7}$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct PhoneExt(::std::string::String);
    impl ::std::ops::Deref for PhoneExt {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<PhoneExt> for ::std::string::String {
        fn from(value: PhoneExt) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for PhoneExt {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 7usize {
                return Err("longer than 7 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^\\d{0,7}$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^\\d{0,7}$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for PhoneExt {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PhoneExt {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PhoneExt {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for PhoneExt {
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

    ///Money amount with currency
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Money amount with currency",
    ///  "type": "object",
    ///  "required": [
    ///    "amount",
    ///    "currency"
    ///  ],
    ///  "properties": {
    ///    "amount": {
    ///      "description": "Amount as string to preserve precision",
    ///      "examples": [
    ///        "10.99"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 20,
    ///      "pattern": "^[0-9]+(\\.[0-9]{1,2})?$"
    ///    },
    ///    "currency": {
    ///      "description": "Currency code (ISO 4217)",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/Currency"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct Price {
        ///Amount as string to preserve precision
        pub amount: PriceAmount,
        ///Currency code (ISO 4217)
        pub currency: Currency,
    }

    ///Amount as string to preserve precision
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Amount as string to preserve precision",
    ///  "examples": [
    ///    "10.99"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 20,
    ///  "pattern": "^[0-9]+(\\.[0-9]{1,2})?$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct PriceAmount(::std::string::String);
    impl ::std::ops::Deref for PriceAmount {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<PriceAmount> for ::std::string::String {
        fn from(value: PriceAmount) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for PriceAmount {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 20usize {
                return Err("longer than 20 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^[0-9]+(\\.[0-9]{1,2})?$").unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[0-9]+(\\.[0-9]{1,2})?$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for PriceAmount {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PriceAmount {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PriceAmount {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for PriceAmount {
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

    ///Money amount with currency
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Money amount with currency",
    ///  "type": "object",
    ///  "properties": {
    ///    "amount": {
    ///      "description": "Amount as string to preserve precision",
    ///      "examples": [
    ///        "10.99"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 20,
    ///      "pattern": "^[0-9]+(\\.[0-9]{1,2})?$"
    ///    },
    ///    "currency": {
    ///      "description": "Currency code (ISO 4217)",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/Currency"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PriceUpdate {
        ///Amount as string to preserve precision
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub amount: ::std::option::Option<PriceUpdateAmount>,
        ///Currency code (ISO 4217)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub currency: ::std::option::Option<Currency>,
    }

    impl ::std::default::Default for PriceUpdate {
        fn default() -> Self {
            Self {
                amount: Default::default(),
                currency: Default::default(),
            }
        }
    }

    ///Amount as string to preserve precision
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Amount as string to preserve precision",
    ///  "examples": [
    ///    "10.99"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 20,
    ///  "pattern": "^[0-9]+(\\.[0-9]{1,2})?$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct PriceUpdateAmount(::std::string::String);
    impl ::std::ops::Deref for PriceUpdateAmount {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<PriceUpdateAmount> for ::std::string::String {
        fn from(value: PriceUpdateAmount) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for PriceUpdateAmount {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 20usize {
                return Err("longer than 20 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^[0-9]+(\\.[0-9]{1,2})?$").unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[0-9]+(\\.[0-9]{1,2})?$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for PriceUpdateAmount {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PriceUpdateAmount {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PriceUpdateAmount {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for PriceUpdateAmount {
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

    ///`Provider`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "basic",
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
    pub enum Provider {
        #[serde(rename = "basic")]
        Basic,
        #[serde(rename = "custom")]
        Custom,
    }

    impl ::std::fmt::Display for Provider {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Basic => f.write_str("basic"),
                Self::Custom => f.write_str("custom"),
            }
        }
    }

    impl ::std::str::FromStr for Provider {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "basic" => Ok(Self::Basic),
                "custom" => Ok(Self::Custom),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for Provider {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for Provider {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for Provider {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to map an IP address to its corresponding domain name in a reverse
    /// DNS lookup
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map an IP address to its corresponding domain
    /// name in a reverse DNS lookup",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "pointer",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "pointer": {
    ///      "description": "The domain name that corresponds to the given IP
    /// address",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "PTR"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PtrResourceRecord {
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///The domain name that corresponds to the given IP address
        pub pointer: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: PtrResourceRecordType,
    }

    ///Used to map an IP address to its corresponding domain name in a reverse
    /// DNS lookup
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map an IP address to its corresponding domain
    /// name in a reverse DNS lookup",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "pointer",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "pointer": {
    ///      "description": "The domain name that corresponds to the given IP
    /// address",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "PTR"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PtrResourceRecordCreateOrUpdateItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///The domain name that corresponds to the given IP address
        pub pointer: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: PtrResourceRecordCreateOrUpdateItemType,
    }

    ///`PtrResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "PTR"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum PtrResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "PTR")]
        Ptr,
    }

    impl ::std::fmt::Display for PtrResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Ptr => f.write_str("PTR"),
            }
        }
    }

    impl ::std::str::FromStr for PtrResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "PTR" => Ok(Self::Ptr),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PtrResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PtrResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PtrResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to map an IP address to its corresponding domain name in a reverse
    /// DNS lookup
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to map an IP address to its corresponding domain
    /// name in a reverse DNS lookup",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "pointer",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "pointer": {
    ///      "description": "The domain name that corresponds to the given IP
    /// address",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "PTR"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct PtrResourceRecordDeleteItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///The domain name that corresponds to the given IP address
        pub pointer: HostNameValue,
        #[serde(rename = "type")]
        pub type_: PtrResourceRecordDeleteItemType,
    }

    ///`PtrResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "PTR"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum PtrResourceRecordDeleteItemType {
        #[serde(rename = "PTR")]
        Ptr,
    }

    impl ::std::fmt::Display for PtrResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Ptr => f.write_str("PTR"),
            }
        }
    }

    impl ::std::str::FromStr for PtrResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "PTR" => Ok(Self::Ptr),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PtrResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PtrResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PtrResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`PtrResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "PTR"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum PtrResourceRecordType {
        #[serde(rename = "PTR")]
        Ptr,
    }

    impl ::std::fmt::Display for PtrResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Ptr => f.write_str("PTR"),
            }
        }
    }

    impl ::std::str::FromStr for PtrResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "PTR" => Ok(Self::Ptr),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for PtrResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for PtrResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for PtrResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`RateLimitError`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "detail"
    ///  ],
    ///  "properties": {
    ///    "detail": {
    ///      "description": "A general message about the exception",
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "pattern": "^[\\s|\\S]*$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RateLimitError {
        ///A general message about the exception
        pub detail: RateLimitErrorDetail,
    }

    ///A general message about the exception
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A general message about the exception",
    ///  "readOnly": true,
    ///  "type": "string",
    ///  "pattern": "^[\\s|\\S]*$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct RateLimitErrorDetail(::std::string::String);
    impl ::std::ops::Deref for RateLimitErrorDetail {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<RateLimitErrorDetail> for ::std::string::String {
        fn from(value: RateLimitErrorDetail) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for RateLimitErrorDetail {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[\\s|\\S]*$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\s|\\S]*$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for RateLimitErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for RateLimitErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for RateLimitErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for RateLimitErrorDetail {
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

    ///Contact ID
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contact ID",
    ///  "examples": [
    ///    "1ZdMXpapqp9sle5dl8BlppTJXAzf5"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32,
    ///  "minLength": 27,
    ///  "pattern": "[a-zA-Z0-9]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ReadAttributeDetailsContact(::std::string::String);
    impl ::std::ops::Deref for ReadAttributeDetailsContact {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ReadAttributeDetailsContact> for ::std::string::String {
        fn from(value: ReadAttributeDetailsContact) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ReadAttributeDetailsContact {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            if value.chars().count() < 27usize {
                return Err("shorter than 27 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("[a-zA-Z0-9]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"[a-zA-Z0-9]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ReadAttributeDetailsContact {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ReadAttributeDetailsContact {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ReadAttributeDetailsContact {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ReadAttributeDetailsContact {
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

    ///Contact ID
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contact ID",
    ///  "examples": [
    ///    "1ZdMXpapqp9sle5dl8BlppTJXAzf5"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 32,
    ///  "minLength": 27,
    ///  "pattern": "[a-zA-Z0-9]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ReadDetailsContact(::std::string::String);
    impl ::std::ops::Deref for ReadDetailsContact {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ReadDetailsContact> for ::std::string::String {
        fn from(value: ReadDetailsContact) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ReadDetailsContact {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            if value.chars().count() < 27usize {
                return Err("shorter than 27 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("[a-zA-Z0-9]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"[a-zA-Z0-9]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ReadDetailsContact {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ReadDetailsContact {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ReadDetailsContact {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ReadDetailsContact {
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

    ///Resource records list query params
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Resource records list query params",
    ///  "type": "object",
    ///  "required": [
    ///    "skip",
    ///    "take"
    ///  ],
    ///  "properties": {
    ///    "orderBy": {
    ///      "description": "Specifies fields and order to sort the response
    /// items",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "enum": [
    ///          "type",
    ///          "-type",
    ///          "name",
    ///          "-name"
    ///        ]
    ///      },
    ///      "maxItems": 1
    ///    },
    ///    "skip": {
    ///      "description": "Number of response items to skip",
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 2147483647.0,
    ///      "minimum": 0.0
    ///    },
    ///    "take": {
    ///      "description": "Number of response items per page",
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 500.0,
    ///      "minimum": 1.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RecordsGetResourceRecordsListQueryParams {
        ///Specifies fields and order to sort the response items
        #[serde(
            rename = "orderBy",
            default,
            skip_serializing_if = "::std::vec::Vec::is_empty"
        )]
        pub order_by: ::std::vec::Vec<RecordsGetResourceRecordsListQueryParamsOrderByItem>,
        ///Number of response items to skip
        pub skip: i32,
        ///Number of response items per page
        pub take: ::std::num::NonZeroU32,
    }

    ///`RecordsGetResourceRecordsListQueryParamsOrderByItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "type",
    ///    "-type",
    ///    "name",
    ///    "-name"
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
    pub enum RecordsGetResourceRecordsListQueryParamsOrderByItem {
        #[serde(rename = "type")]
        Type,
        #[serde(rename = "-type")]
        Xtype,
        #[serde(rename = "name")]
        Name,
        #[serde(rename = "-name")]
        Xname,
    }

    impl ::std::fmt::Display for RecordsGetResourceRecordsListQueryParamsOrderByItem {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Type => f.write_str("type"),
                Self::Xtype => f.write_str("-type"),
                Self::Name => f.write_str("name"),
                Self::Xname => f.write_str("-name"),
            }
        }
    }

    impl ::std::str::FromStr for RecordsGetResourceRecordsListQueryParamsOrderByItem {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "type" => Ok(Self::Type),
                "-type" => Ok(Self::Xtype),
                "name" => Ok(Self::Name),
                "-name" => Ok(Self::Xname),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for RecordsGetResourceRecordsListQueryParamsOrderByItem {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for RecordsGetResourceRecordsListQueryParamsOrderByItem
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for RecordsGetResourceRecordsListQueryParamsOrderByItem
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`RecordsRecordsUpdateModel`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "items"
    ///  ],
    ///  "properties": {
    ///    "force": {
    ///      "description": "Turn-off conflicts resolution checker and force
    /// zone update",
    ///      "type": "boolean"
    ///    },
    ///    "items": {
    ///      "$ref":
    /// "#/components/schemas/ResourceRecordsListCreateOrUpdateItem"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct RecordsRecordsUpdateModel {
        ///Turn-off conflicts resolution checker and force zone update
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub force: ::std::option::Option<bool>,
        pub items: ResourceRecordsListCreateOrUpdateItem,
    }

    ///DNS resource record details
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS resource record details",
    ///  "type": "object",
    ///  "required": [
    ///    "group",
    ///    "name",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "group": {
    ///      "$ref": "#/components/schemas/ResourceRecordsGroup"
    ///    },
    ///    "name": {
    ///      "description": "Name of resource record excluding domain name part.
    /// '@' can be used as an apex domain",
    ///      "examples": [
    ///        "@"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "ttl": {
    ///      "description": "Specifies the amount of time in seconds that a DNS record should be cached by a resolver\nor a caching server before it expires and needs to be refreshed from the authoritative DNS servers",
    ///      "examples": [
    ///        3600
    ///      ],
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 3600.0,
    ///      "minimum": 60.0
    ///    },
    ///    "type": {
    ///      "examples": [
    ///        "A"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 5,
    ///      "minLength": 1,
    ///      "pattern": "\\w+"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ResourceRecord {
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: ResourceRecordType,
    }

    ///DNS resource record details
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS resource record details",
    ///  "type": "object",
    ///  "required": [
    ///    "name",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "name": {
    ///      "description": "Name of resource record excluding domain name part.
    /// '@' can be used as an apex domain",
    ///      "examples": [
    ///        "@"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "ttl": {
    ///      "description": "Specifies the amount of time in seconds that a DNS record should be cached by a resolver\nor a caching server before it expires and needs to be refreshed from the authoritative DNS servers",
    ///      "examples": [
    ///        3600
    ///      ],
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 3600.0,
    ///      "minimum": 60.0
    ///    },
    ///    "type": {
    ///      "examples": [
    ///        "A"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 5,
    ///      "minLength": 1,
    ///      "pattern": "\\w+"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ResourceRecordCreateOrUpdateItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: ResourceRecordCreateOrUpdateItemType,
    }

    ///`ResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    "A"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ResourceRecordCreateOrUpdateItemType(::std::string::String);
    impl ::std::ops::Deref for ResourceRecordCreateOrUpdateItemType {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ResourceRecordCreateOrUpdateItemType> for ::std::string::String {
        fn from(value: ResourceRecordCreateOrUpdateItemType) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5usize {
                return Err("longer than 5 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("\\w+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"\\w+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ResourceRecordCreateOrUpdateItemType {
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

    ///DNS resource record details
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "DNS resource record details",
    ///  "type": "object",
    ///  "required": [
    ///    "name",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "name": {
    ///      "description": "Name of resource record excluding domain name part.
    /// '@' can be used as an apex domain",
    ///      "examples": [
    ///        "@"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "examples": [
    ///        "A"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 5,
    ///      "minLength": 1,
    ///      "pattern": "\\w+"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ResourceRecordDeleteItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        #[serde(rename = "type")]
        pub type_: ResourceRecordDeleteItemType,
    }

    ///`ResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    "A"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ResourceRecordDeleteItemType(::std::string::String);
    impl ::std::ops::Deref for ResourceRecordDeleteItemType {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ResourceRecordDeleteItemType> for ::std::string::String {
        fn from(value: ResourceRecordDeleteItemType) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5usize {
                return Err("longer than 5 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("\\w+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"\\w+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ResourceRecordDeleteItemType {
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

    ///`ResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    "A"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ResourceRecordType(::std::string::String);
    impl ::std::ops::Deref for ResourceRecordType {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ResourceRecordType> for ::std::string::String {
        fn from(value: ResourceRecordType) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5usize {
                return Err("longer than 5 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("\\w+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"\\w+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ResourceRecordType {
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

    ///Details of the group record belongs to
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Details of the group record belongs to",
    ///  "type": "object",
    ///  "required": [
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "type": {
    ///      "description": "* custom: record belongs to custom resource records
    /// group\n* product: record was added as a part of products connection\n*
    /// personalNs: record is associated with the corresponding personal
    /// nameserver",
    ///      "examples": [
    ///        "custom"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "custom",
    ///        "product",
    ///        "personalNs"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ResourceRecordsGroup {
        /// * custom: record belongs to custom resource records group
        /// * product: record was added as a part of products connection
        /// * personalNs: record is associated with the corresponding personal
        ///   nameserver
        #[serde(rename = "type")]
        pub type_: ResourceRecordsGroupType,
    }

    /// * custom: record belongs to custom resource records group
    /// * product: record was added as a part of products connection
    /// * personalNs: record is associated with the corresponding personal
    ///   nameserver
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "* custom: record belongs to custom resource records
    /// group\n* product: record was added as a part of products connection\n*
    /// personalNs: record is associated with the corresponding personal
    /// nameserver",
    ///  "examples": [
    ///    "custom"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "custom",
    ///    "product",
    ///    "personalNs"
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
    pub enum ResourceRecordsGroupType {
        #[serde(rename = "custom")]
        Custom,
        #[serde(rename = "product")]
        Product,
        #[serde(rename = "personalNs")]
        PersonalNs,
    }

    impl ::std::fmt::Display for ResourceRecordsGroupType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Custom => f.write_str("custom"),
                Self::Product => f.write_str("product"),
                Self::PersonalNs => f.write_str("personalNs"),
            }
        }
    }

    impl ::std::str::FromStr for ResourceRecordsGroupType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "custom" => Ok(Self::Custom),
                "product" => Ok(Self::Product),
                "personalNs" => Ok(Self::PersonalNs),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for ResourceRecordsGroupType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ResourceRecordsGroupType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ResourceRecordsGroupType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`ResourceRecordsListCreateOrUpdateItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "array",
    ///  "items": {
    ///    "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///  },
    ///  "maxItems": 500,
    ///  "minItems": 1
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct ResourceRecordsListCreateOrUpdateItem(
        pub ::std::vec::Vec<ResourceRecordCreateOrUpdateItem>,
    );
    impl ::std::ops::Deref for ResourceRecordsListCreateOrUpdateItem {
        type Target = ::std::vec::Vec<ResourceRecordCreateOrUpdateItem>;
        fn deref(&self) -> &::std::vec::Vec<ResourceRecordCreateOrUpdateItem> {
            &self.0
        }
    }

    impl ::std::convert::From<ResourceRecordsListCreateOrUpdateItem>
        for ::std::vec::Vec<ResourceRecordCreateOrUpdateItem>
    {
        fn from(value: ResourceRecordsListCreateOrUpdateItem) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::vec::Vec<ResourceRecordCreateOrUpdateItem>>
        for ResourceRecordsListCreateOrUpdateItem
    {
        fn from(value: ::std::vec::Vec<ResourceRecordCreateOrUpdateItem>) -> Self {
            Self(value)
        }
    }

    ///`ResourceRecordsListDeleteItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "array",
    ///  "items": {
    ///    "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///  },
    ///  "maxItems": 500,
    ///  "minItems": 1
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct ResourceRecordsListDeleteItem(pub ::std::vec::Vec<ResourceRecordDeleteItem>);
    impl ::std::ops::Deref for ResourceRecordsListDeleteItem {
        type Target = ::std::vec::Vec<ResourceRecordDeleteItem>;
        fn deref(&self) -> &::std::vec::Vec<ResourceRecordDeleteItem> {
            &self.0
        }
    }

    impl ::std::convert::From<ResourceRecordsListDeleteItem>
        for ::std::vec::Vec<ResourceRecordDeleteItem>
    {
        fn from(value: ResourceRecordsListDeleteItem) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::vec::Vec<ResourceRecordDeleteItem>>
        for ResourceRecordsListDeleteItem
    {
        fn from(value: ::std::vec::Vec<ResourceRecordDeleteItem>) -> Self {
            Self(value)
        }
    }

    ///`SellerHubDomainResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "name",
    ///    "status",
    ///    "unicodeName"
    ///  ],
    ///  "properties": {
    ///    "binPrice": {
    ///      "description": "Buy It Now (BIN) price for the domain",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/Price"
    ///        }
    ///      ]
    ///    },
    ///    "binPriceEnabled": {
    ///      "description": "Indicates whether the Buy It Now (BIN) option is
    /// enabled",
    ///      "type": "boolean"
    ///    },
    ///    "description": {
    ///      "description": "Domain description",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/domainDescription"
    ///        }
    ///      ]
    ///    },
    ///    "displayName": {
    ///      "description": "Full domain name with original capitalization
    /// setup",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/displayName"
    ///        }
    ///      ]
    ///    },
    ///    "minPrice": {
    ///      "description": "Minimum offer price for the domain",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/Price"
    ///        }
    ///      ]
    ///    },
    ///    "minPriceEnabled": {
    ///      "description": "Indicates whether offer negotiation with minimum
    /// price is enabled",
    ///      "type": "boolean"
    ///    },
    ///    "name": {
    ///      "examples": [
    ///        "xn--spceship-9ya.com"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/sellerhubDomainName"
    ///        }
    ///      ]
    ///    },
    ///    "status": {
    ///      "description": "Current status of the domain in SellerHub",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/SellerHubDomainStatus"
    ///        }
    ///      ]
    ///    },
    ///    "unicodeName": {
    ///      "examples": [
    ///        "spaceship.com"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/domainNameULabel"
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SellerHubDomainResponse {
        ///Buy It Now (BIN) price for the domain
        #[serde(
            rename = "binPrice",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub bin_price: ::std::option::Option<Price>,
        ///Indicates whether the Buy It Now (BIN) option is enabled
        #[serde(
            rename = "binPriceEnabled",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub bin_price_enabled: ::std::option::Option<bool>,
        ///Domain description
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub description: ::std::option::Option<DomainDescription>,
        ///Full domain name with original capitalization setup
        #[serde(
            rename = "displayName",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub display_name: ::std::option::Option<DisplayName>,
        ///Minimum offer price for the domain
        #[serde(
            rename = "minPrice",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub min_price: ::std::option::Option<Price>,
        ///Indicates whether offer negotiation with minimum price is enabled
        #[serde(
            rename = "minPriceEnabled",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub min_price_enabled: ::std::option::Option<bool>,
        pub name: SellerhubDomainName,
        ///Current status of the domain in SellerHub
        pub status: SellerHubDomainStatus,
        #[serde(rename = "unicodeName")]
        pub unicode_name: DomainNameULabel,
    }

    ///SellerHub domain status
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "SellerHub domain status",
    ///  "type": "string",
    ///  "enum": [
    ///    "failed",
    ///    "verifying",
    ///    "onSale",
    ///    "onSaleStopped",
    ///    "saleProcessing",
    ///    "leaseActive",
    ///    "sold"
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
    pub enum SellerHubDomainStatus {
        #[serde(rename = "failed")]
        Failed,
        #[serde(rename = "verifying")]
        Verifying,
        #[serde(rename = "onSale")]
        OnSale,
        #[serde(rename = "onSaleStopped")]
        OnSaleStopped,
        #[serde(rename = "saleProcessing")]
        SaleProcessing,
        #[serde(rename = "leaseActive")]
        LeaseActive,
        #[serde(rename = "sold")]
        Sold,
    }

    impl ::std::fmt::Display for SellerHubDomainStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Failed => f.write_str("failed"),
                Self::Verifying => f.write_str("verifying"),
                Self::OnSale => f.write_str("onSale"),
                Self::OnSaleStopped => f.write_str("onSaleStopped"),
                Self::SaleProcessing => f.write_str("saleProcessing"),
                Self::LeaseActive => f.write_str("leaseActive"),
                Self::Sold => f.write_str("sold"),
            }
        }
    }

    impl ::std::str::FromStr for SellerHubDomainStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "failed" => Ok(Self::Failed),
                "verifying" => Ok(Self::Verifying),
                "onSale" => Ok(Self::OnSale),
                "onSaleStopped" => Ok(Self::OnSaleStopped),
                "saleProcessing" => Ok(Self::SaleProcessing),
                "leaseActive" => Ok(Self::LeaseActive),
                "sold" => Ok(Self::Sold),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SellerHubDomainStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SellerHubDomainStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SellerHubDomainStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Seller Hub Domains List Query Params
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Seller Hub Domains List Query Params",
    ///  "type": "object",
    ///  "required": [
    ///    "skip",
    ///    "take"
    ///  ],
    ///  "properties": {
    ///    "skip": {
    ///      "description": "Number of response items to skip",
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 2147483647.0,
    ///      "minimum": 0.0
    ///    },
    ///    "take": {
    ///      "description": "Number of response items per page",
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "maximum": 100.0,
    ///      "minimum": 1.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SellerHubGetSellerDomainListQueryParams {
        ///Number of response items to skip
        pub skip: i32,
        ///Number of response items per page
        pub take: ::std::num::NonZeroU32,
    }

    ///`SellerHubGetSellerDomainListResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "contactId"
    ///  ],
    ///  "properties": {
    ///    "contactId": {
    ///      "description": "Response with contactId generated, if contact was
    /// created, or existing contact's one.",
    ///      "type": "string",
    ///      "maxLength": 32,
    ///      "minLength": 27,
    ///      "pattern": "[a-zA-Z0-9]+"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SellerHubGetSellerDomainListResponse {
        ///Response with contactId generated, if contact was created, or
        /// existing contact's one.
        #[serde(rename = "contactId")]
        pub contact_id: SellerHubGetSellerDomainListResponseContactId,
    }

    ///Response with contactId generated, if contact was created, or existing
    /// contact's one.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Response with contactId generated, if contact was
    /// created, or existing contact's one.",
    ///  "type": "string",
    ///  "maxLength": 32,
    ///  "minLength": 27,
    ///  "pattern": "[a-zA-Z0-9]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SellerHubGetSellerDomainListResponseContactId(::std::string::String);
    impl ::std::ops::Deref for SellerHubGetSellerDomainListResponseContactId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SellerHubGetSellerDomainListResponseContactId> for ::std::string::String {
        fn from(value: SellerHubGetSellerDomainListResponseContactId) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SellerHubGetSellerDomainListResponseContactId {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            if value.chars().count() < 27usize {
                return Err("shorter than 27 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("[a-zA-Z0-9]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"[a-zA-Z0-9]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SellerHubGetSellerDomainListResponseContactId {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for SellerHubGetSellerDomainListResponseContactId
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for SellerHubGetSellerDomainListResponseContactId
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SellerHubGetSellerDomainListResponseContactId {
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

    ///Represents one verification method option.
    ///Each option contains one or more DNS records that must all be created
    /// together. Users can choose any ONE option from the available options
    /// to verify domain ownership.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Represents one verification method option.\nEach option
    /// contains one or more DNS records that must all be created
    /// together.\nUsers can choose any ONE option from the available options to
    /// verify domain ownership.",
    ///  "type": "object",
    ///  "required": [
    ///    "records"
    ///  ],
    ///  "properties": {
    ///    "records": {
    ///      "description": "Array of DNS records that must all be created for
    /// this verification option.\nAll records in this array are required (AND
    /// logic).",
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/SellerHub.VerificationRecord"
    ///      },
    ///      "maxItems": 10,
    ///      "minItems": 1
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SellerHubVerificationOption {
        ///Array of DNS records that must all be created for this verification
        /// option. All records in this array are required (AND logic).
        pub records: ::std::vec::Vec<SellerHubVerificationRecord>,
    }

    ///Verification record response
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Verification record response",
    ///  "type": "object",
    ///  "required": [
    ///    "name",
    ///    "type",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "name": {
    ///      "description": "Name of resource record excluding domain name part.
    /// '@' can be used as an apex domain.\nUse this value as the 'Name' or
    /// 'Host' field when creating the DNS record.\n\nCommon values:\n- '@' -
    /// Applies the record at the apex/root of your domain",
    ///      "examples": [
    ///        "@"
    ///      ],
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "description": "Type of DNS resource record to create for domain
    /// verification",
    ///      "examples": [
    ///        "TXT"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 5,
    ///      "minLength": 1,
    ///      "pattern": "\\w+"
    ///    },
    ///    "value": {
    ///      "description": "Verification value to be set as the DNS record
    /// value. Format depends on record type (token for TXT, IP for A/AAAA,
    /// etc.).",
    ///      "examples": [
    ///        "7e018923-c48d-4579-1111-1b9bb6952c96"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 256,
    ///      "pattern": "^[a-zA-Z0-9._:/-]+$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SellerHubVerificationRecord {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain. Use this value as the 'Name' or 'Host'
        /// field when creating the DNS record.
        ///
        ///Common values:
        /// - '@' - Applies the record at the apex/root of your domain
        pub name: HostNameValue,
        ///Type of DNS resource record to create for domain verification
        #[serde(rename = "type")]
        pub type_: SellerHubVerificationRecordType,
        ///Verification value to be set as the DNS record value. Format depends
        /// on record type (token for TXT, IP for A/AAAA, etc.).
        pub value: SellerHubVerificationRecordValue,
    }

    ///Type of DNS resource record to create for domain verification
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Type of DNS resource record to create for domain
    /// verification",
    ///  "examples": [
    ///    "TXT"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SellerHubVerificationRecordType(::std::string::String);
    impl ::std::ops::Deref for SellerHubVerificationRecordType {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SellerHubVerificationRecordType> for ::std::string::String {
        fn from(value: SellerHubVerificationRecordType) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SellerHubVerificationRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 5usize {
                return Err("longer than 5 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("\\w+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"\\w+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SellerHubVerificationRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SellerHubVerificationRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SellerHubVerificationRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SellerHubVerificationRecordType {
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

    ///Verification value to be set as the DNS record value. Format depends on
    /// record type (token for TXT, IP for A/AAAA, etc.).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Verification value to be set as the DNS record value. Format depends on record type (token for TXT, IP for A/AAAA, etc.).",
    ///  "examples": [
    ///    "7e018923-c48d-4579-1111-1b9bb6952c96"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 256,
    ///  "pattern": "^[a-zA-Z0-9._:/-]+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SellerHubVerificationRecordValue(::std::string::String);
    impl ::std::ops::Deref for SellerHubVerificationRecordValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SellerHubVerificationRecordValue> for ::std::string::String {
        fn from(value: SellerHubVerificationRecordValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SellerHubVerificationRecordValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 256usize {
                return Err("longer than 256 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^[a-zA-Z0-9._:/-]+$").unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[a-zA-Z0-9._:/-]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SellerHubVerificationRecordValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SellerHubVerificationRecordValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SellerHubVerificationRecordValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SellerHubVerificationRecordValue {
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

    ///Verification response containing one or more verification options.
    ///Users can choose to implement ANY ONE of the provided options (OR
    /// logic). Within each option, ALL records must be created (AND logic).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Verification response containing one or more
    /// verification options.\nUsers can choose to implement ANY ONE of the
    /// provided options (OR logic).\nWithin each option, ALL records must be
    /// created (AND logic).",
    ///  "examples": [
    ///    {
    ///      "options": [
    ///        {
    ///          "records": [
    ///            {
    ///              "name": "@",
    ///              "type": "TXT",
    ///              "value": "spaceship-verification-token-abc123"
    ///            }
    ///          ]
    ///        },
    ///        {
    ///          "records": [
    ///            {
    ///              "name": "_spaceship_verify",
    ///              "type": "A",
    ///              "value": "192.0.2.1"
    ///            },
    ///            {
    ///              "name": "_spaceship_verify",
    ///              "type": "AAAA",
    ///              "value": "2001:db8::1"
    ///            }
    ///          ]
    ///        }
    ///      ]
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "required": [
    ///    "options"
    ///  ],
    ///  "properties": {
    ///    "options": {
    ///      "description": "Array of verification options available for domain
    /// verification.\n\n**OR Logic:** Choose any ONE option from this array to
    /// verify your domain.\n**AND Logic:** Within the chosen option, create ALL
    /// records listed.\n\nIf only one option is provided, that is the required
    /// verification method.\nIf multiple options are provided, you can choose
    /// the most convenient method for your setup.",
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/SellerHub.VerificationOption"
    ///      },
    ///      "maxItems": 5,
    ///      "minItems": 1
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SellerHubVerificationResponse {
        ///Array of verification options available for domain verification.
        ///
        ///**OR Logic:** Choose any ONE option from this array to verify your
        /// domain. **AND Logic:** Within the chosen option, create ALL
        /// records listed.
        ///
        ///If only one option is provided, that is the required verification
        /// method. If multiple options are provided, you can choose the
        /// most convenient method for your setup.
        pub options: ::std::vec::Vec<SellerHubVerificationOption>,
    }

    ///SellerHub Domain Name
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "SellerHub Domain Name",
    ///  "examples": [
    ///    "onsale.com"
    ///  ],
    ///  "type": "string",
    ///  "format": "domain",
    ///  "maxLength": 255,
    ///  "minLength": 4
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
    pub struct SellerhubDomainName(pub ::std::string::String);
    impl ::std::ops::Deref for SellerhubDomainName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SellerhubDomainName> for ::std::string::String {
        fn from(value: SellerhubDomainName) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::std::string::String> for SellerhubDomainName {
        fn from(value: ::std::string::String) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for SellerhubDomainName {
        type Err = ::std::convert::Infallible;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::fmt::Display for SellerhubDomainName {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    ///Used to specify the location of servers for specific services
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to specify the location of servers for specific
    /// services",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "port",
    ///    "priority",
    ///    "protocol",
    ///    "service",
    ///    "target",
    ///    "type",
    ///    "weight"
    ///  ],
    ///  "properties": {
    ///    "port": {
    ///      "description": "The port number on which the service is
    /// available.",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 1.0
    ///    },
    ///    "priority": {
    ///      "description": "An integer that indicates the priority of the
    /// target host, with lower values indicating higher priority",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "protocol": {
    ///      "description": "Indicates the transport protocol the service uses,
    /// such as \"_tcp\" for TCP or \"_udp\" for UDP",
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "minLength": 2,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "service": {
    ///      "description": "Specifies the symbolic name of the desired
    /// service.\nFor example, \"_sip\" for SIP (Session Initiation Protocol) or
    /// \"_ldap\" for LDAP (Lightweight Directory Access Protocol)",
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "minLength": 2,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "target": {
    ///      "description": "The domain name of the server providing the
    /// service.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "SRV"
    ///      ]
    ///    },
    ///    "weight": {
    ///      "description": "Used in conjunction with the Priority field to load
    /// balance between multiple targets with the same priority.\nHigher values
    /// receive more connections.",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SrvResourceRecord {
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///The port number on which the service is available.
        pub port: ::std::num::NonZeroU16,
        ///An integer that indicates the priority of the target host, with
        /// lower values indicating higher priority
        pub priority: u16,
        ///Indicates the transport protocol the service uses, such as "_tcp"
        /// for TCP or "_udp" for UDP
        pub protocol: SrvResourceRecordProtocol,
        ///Specifies the symbolic name of the desired service.
        ///For example, "_sip" for SIP (Session Initiation Protocol) or "_ldap"
        /// for LDAP (Lightweight Directory Access Protocol)
        pub service: SrvResourceRecordService,
        ///The domain name of the server providing the service.
        pub target: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: SrvResourceRecordType,
        ///Used in conjunction with the Priority field to load balance between
        /// multiple targets with the same priority. Higher values
        /// receive more connections.
        pub weight: u16,
    }

    ///Used to specify the location of servers for specific services
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to specify the location of servers for specific
    /// services",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "port",
    ///    "priority",
    ///    "protocol",
    ///    "service",
    ///    "target",
    ///    "type",
    ///    "weight"
    ///  ],
    ///  "properties": {
    ///    "port": {
    ///      "description": "The port number on which the service is
    /// available.",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 1.0
    ///    },
    ///    "priority": {
    ///      "description": "An integer that indicates the priority of the
    /// target host, with lower values indicating higher priority",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "protocol": {
    ///      "description": "Indicates the transport protocol the service uses,
    /// such as \"_tcp\" for TCP or \"_udp\" for UDP",
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "minLength": 2,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "service": {
    ///      "description": "Specifies the symbolic name of the desired
    /// service.\nFor example, \"_sip\" for SIP (Session Initiation Protocol) or
    /// \"_ldap\" for LDAP (Lightweight Directory Access Protocol)",
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "minLength": 2,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "target": {
    ///      "description": "The domain name of the server providing the
    /// service.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "SRV"
    ///      ]
    ///    },
    ///    "weight": {
    ///      "description": "Used in conjunction with the Priority field to load
    /// balance between multiple targets with the same priority.\nHigher values
    /// receive more connections.",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SrvResourceRecordCreateOrUpdateItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///The port number on which the service is available.
        pub port: ::std::num::NonZeroU16,
        ///An integer that indicates the priority of the target host, with
        /// lower values indicating higher priority
        pub priority: u16,
        ///Indicates the transport protocol the service uses, such as "_tcp"
        /// for TCP or "_udp" for UDP
        pub protocol: SrvResourceRecordCreateOrUpdateItemProtocol,
        ///Specifies the symbolic name of the desired service.
        ///For example, "_sip" for SIP (Session Initiation Protocol) or "_ldap"
        /// for LDAP (Lightweight Directory Access Protocol)
        pub service: SrvResourceRecordCreateOrUpdateItemService,
        ///The domain name of the server providing the service.
        pub target: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: SrvResourceRecordCreateOrUpdateItemType,
        ///Used in conjunction with the Priority field to load balance between
        /// multiple targets with the same priority. Higher values
        /// receive more connections.
        pub weight: u16,
    }

    ///Indicates the transport protocol the service uses, such as "_tcp" for
    /// TCP or "_udp" for UDP
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Indicates the transport protocol the service uses, such
    /// as \"_tcp\" for TCP or \"_udp\" for UDP",
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "minLength": 2,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SrvResourceRecordCreateOrUpdateItemProtocol(::std::string::String);
    impl ::std::ops::Deref for SrvResourceRecordCreateOrUpdateItemProtocol {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SrvResourceRecordCreateOrUpdateItemProtocol> for ::std::string::String {
        fn from(value: SrvResourceRecordCreateOrUpdateItemProtocol) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SrvResourceRecordCreateOrUpdateItemProtocol {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SrvResourceRecordCreateOrUpdateItemProtocol {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for SrvResourceRecordCreateOrUpdateItemProtocol
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for SrvResourceRecordCreateOrUpdateItemProtocol
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SrvResourceRecordCreateOrUpdateItemProtocol {
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

    ///Specifies the symbolic name of the desired service.
    ///For example, "_sip" for SIP (Session Initiation Protocol) or "_ldap" for
    /// LDAP (Lightweight Directory Access Protocol)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the symbolic name of the desired
    /// service.\nFor example, \"_sip\" for SIP (Session Initiation Protocol) or
    /// \"_ldap\" for LDAP (Lightweight Directory Access Protocol)",
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "minLength": 2,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SrvResourceRecordCreateOrUpdateItemService(::std::string::String);
    impl ::std::ops::Deref for SrvResourceRecordCreateOrUpdateItemService {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SrvResourceRecordCreateOrUpdateItemService> for ::std::string::String {
        fn from(value: SrvResourceRecordCreateOrUpdateItemService) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SrvResourceRecordCreateOrUpdateItemService {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SrvResourceRecordCreateOrUpdateItemService {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for SrvResourceRecordCreateOrUpdateItemService
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SrvResourceRecordCreateOrUpdateItemService {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SrvResourceRecordCreateOrUpdateItemService {
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

    ///`SrvResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "SRV"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum SrvResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "SRV")]
        Srv,
    }

    impl ::std::fmt::Display for SrvResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Srv => f.write_str("SRV"),
            }
        }
    }

    impl ::std::str::FromStr for SrvResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "SRV" => Ok(Self::Srv),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SrvResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SrvResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SrvResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to specify the location of servers for specific services
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to specify the location of servers for specific
    /// services",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "port",
    ///    "priority",
    ///    "protocol",
    ///    "service",
    ///    "target",
    ///    "type",
    ///    "weight"
    ///  ],
    ///  "properties": {
    ///    "port": {
    ///      "description": "The port number on which the service is
    /// available.",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 1.0
    ///    },
    ///    "priority": {
    ///      "description": "An integer that indicates the priority of the
    /// target host, with lower values indicating higher priority",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "protocol": {
    ///      "description": "Indicates the transport protocol the service uses,
    /// such as \"_tcp\" for TCP or \"_udp\" for UDP",
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "minLength": 2,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "service": {
    ///      "description": "Specifies the symbolic name of the desired
    /// service.\nFor example, \"_sip\" for SIP (Session Initiation Protocol) or
    /// \"_ldap\" for LDAP (Lightweight Directory Access Protocol)",
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "minLength": 2,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "target": {
    ///      "description": "The domain name of the server providing the
    /// service.",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/hostNameValue"
    ///        }
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "SRV"
    ///      ]
    ///    },
    ///    "weight": {
    ///      "description": "Used in conjunction with the Priority field to load
    /// balance between multiple targets with the same priority.\nHigher values
    /// receive more connections.",
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SrvResourceRecordDeleteItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///The port number on which the service is available.
        pub port: ::std::num::NonZeroU16,
        ///An integer that indicates the priority of the target host, with
        /// lower values indicating higher priority
        pub priority: u16,
        ///Indicates the transport protocol the service uses, such as "_tcp"
        /// for TCP or "_udp" for UDP
        pub protocol: SrvResourceRecordDeleteItemProtocol,
        ///Specifies the symbolic name of the desired service.
        ///For example, "_sip" for SIP (Session Initiation Protocol) or "_ldap"
        /// for LDAP (Lightweight Directory Access Protocol)
        pub service: SrvResourceRecordDeleteItemService,
        ///The domain name of the server providing the service.
        pub target: HostNameValue,
        #[serde(rename = "type")]
        pub type_: SrvResourceRecordDeleteItemType,
        ///Used in conjunction with the Priority field to load balance between
        /// multiple targets with the same priority. Higher values
        /// receive more connections.
        pub weight: u16,
    }

    ///Indicates the transport protocol the service uses, such as "_tcp" for
    /// TCP or "_udp" for UDP
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Indicates the transport protocol the service uses, such
    /// as \"_tcp\" for TCP or \"_udp\" for UDP",
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "minLength": 2,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SrvResourceRecordDeleteItemProtocol(::std::string::String);
    impl ::std::ops::Deref for SrvResourceRecordDeleteItemProtocol {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SrvResourceRecordDeleteItemProtocol> for ::std::string::String {
        fn from(value: SrvResourceRecordDeleteItemProtocol) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SrvResourceRecordDeleteItemProtocol {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SrvResourceRecordDeleteItemProtocol {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SrvResourceRecordDeleteItemProtocol {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SrvResourceRecordDeleteItemProtocol {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SrvResourceRecordDeleteItemProtocol {
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

    ///Specifies the symbolic name of the desired service.
    ///For example, "_sip" for SIP (Session Initiation Protocol) or "_ldap" for
    /// LDAP (Lightweight Directory Access Protocol)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the symbolic name of the desired
    /// service.\nFor example, \"_sip\" for SIP (Session Initiation Protocol) or
    /// \"_ldap\" for LDAP (Lightweight Directory Access Protocol)",
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "minLength": 2,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SrvResourceRecordDeleteItemService(::std::string::String);
    impl ::std::ops::Deref for SrvResourceRecordDeleteItemService {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SrvResourceRecordDeleteItemService> for ::std::string::String {
        fn from(value: SrvResourceRecordDeleteItemService) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SrvResourceRecordDeleteItemService {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SrvResourceRecordDeleteItemService {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SrvResourceRecordDeleteItemService {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SrvResourceRecordDeleteItemService {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SrvResourceRecordDeleteItemService {
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

    ///`SrvResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "SRV"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum SrvResourceRecordDeleteItemType {
        #[serde(rename = "SRV")]
        Srv,
    }

    impl ::std::fmt::Display for SrvResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Srv => f.write_str("SRV"),
            }
        }
    }

    impl ::std::str::FromStr for SrvResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "SRV" => Ok(Self::Srv),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SrvResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SrvResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SrvResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Indicates the transport protocol the service uses, such as "_tcp" for
    /// TCP or "_udp" for UDP
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Indicates the transport protocol the service uses, such
    /// as \"_tcp\" for TCP or \"_udp\" for UDP",
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "minLength": 2,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SrvResourceRecordProtocol(::std::string::String);
    impl ::std::ops::Deref for SrvResourceRecordProtocol {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SrvResourceRecordProtocol> for ::std::string::String {
        fn from(value: SrvResourceRecordProtocol) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SrvResourceRecordProtocol {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SrvResourceRecordProtocol {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SrvResourceRecordProtocol {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SrvResourceRecordProtocol {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SrvResourceRecordProtocol {
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

    ///Specifies the symbolic name of the desired service.
    ///For example, "_sip" for SIP (Session Initiation Protocol) or "_ldap" for
    /// LDAP (Lightweight Directory Access Protocol)
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the symbolic name of the desired
    /// service.\nFor example, \"_sip\" for SIP (Session Initiation Protocol) or
    /// \"_ldap\" for LDAP (Lightweight Directory Access Protocol)",
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "minLength": 2,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SrvResourceRecordService(::std::string::String);
    impl ::std::ops::Deref for SrvResourceRecordService {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SrvResourceRecordService> for ::std::string::String {
        fn from(value: SrvResourceRecordService) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SrvResourceRecordService {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SrvResourceRecordService {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SrvResourceRecordService {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SrvResourceRecordService {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SrvResourceRecordService {
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

    ///`SrvResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "SRV"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum SrvResourceRecordType {
        #[serde(rename = "SRV")]
        Srv,
    }

    impl ::std::fmt::Display for SrvResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Srv => f.write_str("SRV"),
            }
        }
    }

    impl ::std::str::FromStr for SrvResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "SRV" => Ok(Self::Srv),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SrvResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SrvResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SrvResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to allow a service to be provided from multiple alternative
    /// endpoints, each with associated parameters.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to allow a service to be provided from multiple
    /// alternative endpoints, each with associated parameters.",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "svcParams",
    ///    "svcPriority",
    ///    "targetName",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "port": {
    ///      "description": "Specifies the port number for which the SVCB record
    /// is applicable.\nIf specified, it must be a single wildcard (an asterisk
    /// symbol) or a string that starts with an underscore and continues with a
    /// number from 1 to 65535.",
    ///      "examples": [
    ///        "_443"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "*"
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/underscoredPort"
    ///        }
    ///      ]
    ///    },
    ///    "scheme": {
    ///      "description": "Indicates the scheme over which the SVCB record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///      "examples": [
    ///        "_tcp"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "svcParams": {
    ///      "description": "A whitespace-separated list with parameters
    /// describing the alternative endpoint at TargetName (only used in
    /// ServiceMode and otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///      "default": "",
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 0,
    ///      "pattern": ".*"
    ///    },
    ///    "svcPriority": {
    ///      "description": "The priority of this record (relative to others,
    /// with lower values preferred).\nWhen svcPriority is 0, the SVCB record is
    /// in AliasMode. Otherwise, it is in ServiceMode.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "targetName": {
    ///      "description": "A fully qualified domain name (FQDN) or a single
    /// \".\", either the alias target (for AliasMode) or the alternative
    /// endpoint (for ServiceMode).\nFor AliasMode, a TargetName of \".\"
    /// indicates that the service is not available or does not exist.\nFor
    /// ServiceMode, if TargetName has the value \".\", then the owner name of
    /// this record is used as the effective TargetName.",
    ///      "examples": [
    ///        "_443._https.www.example.com"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "."
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/fqdn"
    ///        }
    ///      ],
    ///      "maxLength": 253
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "SVCB"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SvcbResourceRecord {
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the port number for which the SVCB record is applicable.
        ///If specified, it must be a single wildcard (an asterisk symbol) or a
        /// string that starts with an underscore and continues with a number
        /// from 1 to 65535.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub port: ::std::option::Option<SvcbResourceRecordPort>,
        ///Indicates the scheme over which the SVCB record applies, such as
        /// "_tcp" for TCP or "_udp" for UDP.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub scheme: ::std::option::Option<SvcbResourceRecordScheme>,
        ///A whitespace-separated list with parameters describing the
        /// alternative endpoint at TargetName (only used in ServiceMode and
        /// otherwise ignored). Each SvcParam consisting of a
        /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.
        ///
        ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
        /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
        /// "tls-supported-groups".
        ///
        ///Arbitrary keys can be represented using the unknown-key presentation
        /// format "keyNNNNN" where NNNNN is the numeric value of the key type
        /// without leading zeros (Number 0-65535).
        #[serde(rename = "svcParams")]
        pub svc_params: SvcbResourceRecordSvcParams,
        ///The priority of this record (relative to others, with lower values
        /// preferred). When svcPriority is 0, the SVCB record is in
        /// AliasMode. Otherwise, it is in ServiceMode.
        #[serde(rename = "svcPriority")]
        pub svc_priority: u16,
        #[serde(rename = "targetName")]
        pub target_name: SvcbResourceRecordTargetName,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: SvcbResourceRecordType,
    }

    ///Used to allow a service to be provided from multiple alternative
    /// endpoints, each with associated parameters.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to allow a service to be provided from multiple
    /// alternative endpoints, each with associated parameters.",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "svcParams",
    ///    "svcPriority",
    ///    "targetName",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "port": {
    ///      "description": "Specifies the port number for which the SVCB record
    /// is applicable.\nIf specified, it must be a single wildcard (an asterisk
    /// symbol) or a string that starts with an underscore and continues with a
    /// number from 1 to 65535.",
    ///      "examples": [
    ///        "_443"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "*"
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/underscoredPort"
    ///        }
    ///      ]
    ///    },
    ///    "scheme": {
    ///      "description": "Indicates the scheme over which the SVCB record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///      "examples": [
    ///        "_tcp"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "svcParams": {
    ///      "description": "A whitespace-separated list with parameters
    /// describing the alternative endpoint at TargetName (only used in
    /// ServiceMode and otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///      "default": "",
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 0,
    ///      "pattern": ".*"
    ///    },
    ///    "svcPriority": {
    ///      "description": "The priority of this record (relative to others,
    /// with lower values preferred).\nWhen svcPriority is 0, the SVCB record is
    /// in AliasMode. Otherwise, it is in ServiceMode.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "targetName": {
    ///      "description": "A fully qualified domain name (FQDN) or a single
    /// \".\", either the alias target (for AliasMode) or the alternative
    /// endpoint (for ServiceMode).\nFor AliasMode, a TargetName of \".\"
    /// indicates that the service is not available or does not exist.\nFor
    /// ServiceMode, if TargetName has the value \".\", then the owner name of
    /// this record is used as the effective TargetName.",
    ///      "examples": [
    ///        "_443._https.www.example.com"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "."
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/fqdn"
    ///        }
    ///      ],
    ///      "maxLength": 253
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "SVCB"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SvcbResourceRecordCreateOrUpdateItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the port number for which the SVCB record is applicable.
        ///If specified, it must be a single wildcard (an asterisk symbol) or a
        /// string that starts with an underscore and continues with a number
        /// from 1 to 65535.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub port: ::std::option::Option<SvcbResourceRecordCreateOrUpdateItemPort>,
        ///Indicates the scheme over which the SVCB record applies, such as
        /// "_tcp" for TCP or "_udp" for UDP.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub scheme: ::std::option::Option<SvcbResourceRecordCreateOrUpdateItemScheme>,
        ///A whitespace-separated list with parameters describing the
        /// alternative endpoint at TargetName (only used in ServiceMode and
        /// otherwise ignored). Each SvcParam consisting of a
        /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.
        ///
        ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
        /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
        /// "tls-supported-groups".
        ///
        ///Arbitrary keys can be represented using the unknown-key presentation
        /// format "keyNNNNN" where NNNNN is the numeric value of the key type
        /// without leading zeros (Number 0-65535).
        #[serde(rename = "svcParams")]
        pub svc_params: SvcbResourceRecordCreateOrUpdateItemSvcParams,
        ///The priority of this record (relative to others, with lower values
        /// preferred). When svcPriority is 0, the SVCB record is in
        /// AliasMode. Otherwise, it is in ServiceMode.
        #[serde(rename = "svcPriority")]
        pub svc_priority: u16,
        #[serde(rename = "targetName")]
        pub target_name: SvcbResourceRecordCreateOrUpdateItemTargetName,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: SvcbResourceRecordCreateOrUpdateItemType,
    }

    ///Specifies the port number for which the SVCB record is applicable.
    ///If specified, it must be a single wildcard (an asterisk symbol) or a
    /// string that starts with an underscore and continues with a number from 1
    /// to 65535.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the port number for which the SVCB record is
    /// applicable.\nIf specified, it must be a single wildcard (an asterisk
    /// symbol) or a string that starts with an underscore and continues with a
    /// number from 1 to 65535.",
    ///  "examples": [
    ///    "_443"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "*"
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/underscoredPort"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SvcbResourceRecordCreateOrUpdateItemPort {
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_0: ::std::option::Option<SvcbResourceRecordCreateOrUpdateItemPortSubtype0>,
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_1: ::std::option::Option<UnderscoredPort>,
    }

    impl ::std::default::Default for SvcbResourceRecordCreateOrUpdateItemPort {
        fn default() -> Self {
            Self {
                subtype_0: Default::default(),
                subtype_1: Default::default(),
            }
        }
    }

    ///`SvcbResourceRecordCreateOrUpdateItemPortSubtype0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "*"
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
    pub enum SvcbResourceRecordCreateOrUpdateItemPortSubtype0 {
        #[serde(rename = "*")]
        X,
    }

    impl ::std::fmt::Display for SvcbResourceRecordCreateOrUpdateItemPortSubtype0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("*"),
            }
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordCreateOrUpdateItemPortSubtype0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "*" => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordCreateOrUpdateItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for SvcbResourceRecordCreateOrUpdateItemPortSubtype0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for SvcbResourceRecordCreateOrUpdateItemPortSubtype0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Indicates the scheme over which the SVCB record applies, such as "_tcp"
    /// for TCP or "_udp" for UDP.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Indicates the scheme over which the SVCB record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///  "examples": [
    ///    "_tcp"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SvcbResourceRecordCreateOrUpdateItemScheme(::std::string::String);
    impl ::std::ops::Deref for SvcbResourceRecordCreateOrUpdateItemScheme {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SvcbResourceRecordCreateOrUpdateItemScheme> for ::std::string::String {
        fn from(value: SvcbResourceRecordCreateOrUpdateItemScheme) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordCreateOrUpdateItemScheme {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordCreateOrUpdateItemScheme {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for SvcbResourceRecordCreateOrUpdateItemScheme
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SvcbResourceRecordCreateOrUpdateItemScheme {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SvcbResourceRecordCreateOrUpdateItemScheme {
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

    ///A whitespace-separated list with parameters describing the alternative
    /// endpoint at TargetName (only used in ServiceMode and otherwise ignored).
    /// Each SvcParam consisting of a SvcParamKey=SvcParamValue pair or a
    /// standalone SvcParamKey.
    ///
    ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
    /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
    /// "tls-supported-groups".
    ///
    ///Arbitrary keys can be represented using the unknown-key presentation
    /// format "keyNNNNN" where NNNNN is the numeric value of the key type
    /// without leading zeros (Number 0-65535).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A whitespace-separated list with parameters describing
    /// the alternative endpoint at TargetName (only used in ServiceMode and
    /// otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///  "default": "",
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 0,
    ///  "pattern": ".*"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SvcbResourceRecordCreateOrUpdateItemSvcParams(::std::string::String);
    impl ::std::ops::Deref for SvcbResourceRecordCreateOrUpdateItemSvcParams {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SvcbResourceRecordCreateOrUpdateItemSvcParams> for ::std::string::String {
        fn from(value: SvcbResourceRecordCreateOrUpdateItemSvcParams) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for SvcbResourceRecordCreateOrUpdateItemSvcParams {
        fn default() -> Self {
            SvcbResourceRecordCreateOrUpdateItemSvcParams("".to_string())
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordCreateOrUpdateItemSvcParams {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 0usize {
                return Err("shorter than 0 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new(".*").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \".*\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordCreateOrUpdateItemSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for SvcbResourceRecordCreateOrUpdateItemSvcParams
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for SvcbResourceRecordCreateOrUpdateItemSvcParams
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SvcbResourceRecordCreateOrUpdateItemSvcParams {
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

    ///A fully qualified domain name (FQDN) or a single ".", either the alias
    /// target (for AliasMode) or the alternative endpoint (for ServiceMode).
    /// For AliasMode, a TargetName of "." indicates that the service is not
    /// available or does not exist. For ServiceMode, if TargetName has the
    /// value ".", then the owner name of this record is used as the effective
    /// TargetName.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A fully qualified domain name (FQDN) or a single \".\",
    /// either the alias target (for AliasMode) or the alternative endpoint (for
    /// ServiceMode).\nFor AliasMode, a TargetName of \".\" indicates that the
    /// service is not available or does not exist.\nFor ServiceMode, if
    /// TargetName has the value \".\", then the owner name of this record is
    /// used as the effective TargetName.",
    ///  "examples": [
    ///    "_443._https.www.example.com"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/fqdn"
    ///    }
    ///  ],
    ///  "maxLength": 253
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum SvcbResourceRecordCreateOrUpdateItemTargetName {
        Variant0(SvcbResourceRecordCreateOrUpdateItemTargetNameVariant0),
        Variant1(::std::string::String),
    }

    impl ::std::fmt::Display for SvcbResourceRecordCreateOrUpdateItemTargetName {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                Self::Variant0(x) => x.fmt(f),
                Self::Variant1(x) => x.fmt(f),
            }
        }
    }

    impl ::std::convert::From<SvcbResourceRecordCreateOrUpdateItemTargetNameVariant0>
        for SvcbResourceRecordCreateOrUpdateItemTargetName
    {
        fn from(value: SvcbResourceRecordCreateOrUpdateItemTargetNameVariant0) -> Self {
            Self::Variant0(value)
        }
    }

    ///`SvcbResourceRecordCreateOrUpdateItemTargetNameVariant0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "maxLength": 253
    ///    },
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "not": {
    ///        "$ref": "#/components/schemas/fqdn"
    ///      }
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
    pub enum SvcbResourceRecordCreateOrUpdateItemTargetNameVariant0 {
        #[serde(rename = ".")]
        X,
    }

    impl ::std::fmt::Display for SvcbResourceRecordCreateOrUpdateItemTargetNameVariant0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("."),
            }
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordCreateOrUpdateItemTargetNameVariant0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "." => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordCreateOrUpdateItemTargetNameVariant0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for SvcbResourceRecordCreateOrUpdateItemTargetNameVariant0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for SvcbResourceRecordCreateOrUpdateItemTargetNameVariant0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`SvcbResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "SVCB"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum SvcbResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "SVCB")]
        Svcb,
    }

    impl ::std::fmt::Display for SvcbResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Svcb => f.write_str("SVCB"),
            }
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "SVCB" => Ok(Self::Svcb),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SvcbResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SvcbResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to allow a service to be provided from multiple alternative
    /// endpoints, each with associated parameters.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to allow a service to be provided from multiple
    /// alternative endpoints, each with associated parameters.",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "svcParams",
    ///    "svcPriority",
    ///    "targetName",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "port": {
    ///      "description": "Specifies the port number for which the SVCB record
    /// is applicable.\nIf specified, it must be a single wildcard (an asterisk
    /// symbol) or a string that starts with an underscore and continues with a
    /// number from 1 to 65535.",
    ///      "examples": [
    ///        "_443"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "*"
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/underscoredPort"
    ///        }
    ///      ]
    ///    },
    ///    "scheme": {
    ///      "description": "Indicates the scheme over which the SVCB record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///      "examples": [
    ///        "_tcp"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "svcParams": {
    ///      "description": "A whitespace-separated list with parameters
    /// describing the alternative endpoint at TargetName (only used in
    /// ServiceMode and otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///      "default": "",
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 0,
    ///      "pattern": ".*"
    ///    },
    ///    "svcPriority": {
    ///      "description": "The priority of this record (relative to others,
    /// with lower values preferred).\nWhen svcPriority is 0, the SVCB record is
    /// in AliasMode. Otherwise, it is in ServiceMode.",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 65535.0,
    ///      "minimum": 0.0
    ///    },
    ///    "targetName": {
    ///      "description": "A fully qualified domain name (FQDN) or a single
    /// \".\", either the alias target (for AliasMode) or the alternative
    /// endpoint (for ServiceMode).\nFor AliasMode, a TargetName of \".\"
    /// indicates that the service is not available or does not exist.\nFor
    /// ServiceMode, if TargetName has the value \".\", then the owner name of
    /// this record is used as the effective TargetName.",
    ///      "examples": [
    ///        "_443._https.www.example.com"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "."
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/fqdn"
    ///        }
    ///      ],
    ///      "maxLength": 253
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "SVCB"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SvcbResourceRecordDeleteItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the port number for which the SVCB record is applicable.
        ///If specified, it must be a single wildcard (an asterisk symbol) or a
        /// string that starts with an underscore and continues with a number
        /// from 1 to 65535.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub port: ::std::option::Option<SvcbResourceRecordDeleteItemPort>,
        ///Indicates the scheme over which the SVCB record applies, such as
        /// "_tcp" for TCP or "_udp" for UDP.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub scheme: ::std::option::Option<SvcbResourceRecordDeleteItemScheme>,
        ///A whitespace-separated list with parameters describing the
        /// alternative endpoint at TargetName (only used in ServiceMode and
        /// otherwise ignored). Each SvcParam consisting of a
        /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.
        ///
        ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
        /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
        /// "tls-supported-groups".
        ///
        ///Arbitrary keys can be represented using the unknown-key presentation
        /// format "keyNNNNN" where NNNNN is the numeric value of the key type
        /// without leading zeros (Number 0-65535).
        #[serde(rename = "svcParams")]
        pub svc_params: SvcbResourceRecordDeleteItemSvcParams,
        ///The priority of this record (relative to others, with lower values
        /// preferred). When svcPriority is 0, the SVCB record is in
        /// AliasMode. Otherwise, it is in ServiceMode.
        #[serde(rename = "svcPriority")]
        pub svc_priority: u16,
        #[serde(rename = "targetName")]
        pub target_name: SvcbResourceRecordDeleteItemTargetName,
        #[serde(rename = "type")]
        pub type_: SvcbResourceRecordDeleteItemType,
    }

    ///Specifies the port number for which the SVCB record is applicable.
    ///If specified, it must be a single wildcard (an asterisk symbol) or a
    /// string that starts with an underscore and continues with a number from 1
    /// to 65535.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the port number for which the SVCB record is
    /// applicable.\nIf specified, it must be a single wildcard (an asterisk
    /// symbol) or a string that starts with an underscore and continues with a
    /// number from 1 to 65535.",
    ///  "examples": [
    ///    "_443"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "*"
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/underscoredPort"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SvcbResourceRecordDeleteItemPort {
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_0: ::std::option::Option<SvcbResourceRecordDeleteItemPortSubtype0>,
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_1: ::std::option::Option<UnderscoredPort>,
    }

    impl ::std::default::Default for SvcbResourceRecordDeleteItemPort {
        fn default() -> Self {
            Self {
                subtype_0: Default::default(),
                subtype_1: Default::default(),
            }
        }
    }

    ///`SvcbResourceRecordDeleteItemPortSubtype0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "*"
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
    pub enum SvcbResourceRecordDeleteItemPortSubtype0 {
        #[serde(rename = "*")]
        X,
    }

    impl ::std::fmt::Display for SvcbResourceRecordDeleteItemPortSubtype0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("*"),
            }
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordDeleteItemPortSubtype0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "*" => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordDeleteItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SvcbResourceRecordDeleteItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SvcbResourceRecordDeleteItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Indicates the scheme over which the SVCB record applies, such as "_tcp"
    /// for TCP or "_udp" for UDP.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Indicates the scheme over which the SVCB record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///  "examples": [
    ///    "_tcp"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SvcbResourceRecordDeleteItemScheme(::std::string::String);
    impl ::std::ops::Deref for SvcbResourceRecordDeleteItemScheme {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SvcbResourceRecordDeleteItemScheme> for ::std::string::String {
        fn from(value: SvcbResourceRecordDeleteItemScheme) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordDeleteItemScheme {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordDeleteItemScheme {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SvcbResourceRecordDeleteItemScheme {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SvcbResourceRecordDeleteItemScheme {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SvcbResourceRecordDeleteItemScheme {
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

    ///A whitespace-separated list with parameters describing the alternative
    /// endpoint at TargetName (only used in ServiceMode and otherwise ignored).
    /// Each SvcParam consisting of a SvcParamKey=SvcParamValue pair or a
    /// standalone SvcParamKey.
    ///
    ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
    /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
    /// "tls-supported-groups".
    ///
    ///Arbitrary keys can be represented using the unknown-key presentation
    /// format "keyNNNNN" where NNNNN is the numeric value of the key type
    /// without leading zeros (Number 0-65535).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A whitespace-separated list with parameters describing
    /// the alternative endpoint at TargetName (only used in ServiceMode and
    /// otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///  "default": "",
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 0,
    ///  "pattern": ".*"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SvcbResourceRecordDeleteItemSvcParams(::std::string::String);
    impl ::std::ops::Deref for SvcbResourceRecordDeleteItemSvcParams {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SvcbResourceRecordDeleteItemSvcParams> for ::std::string::String {
        fn from(value: SvcbResourceRecordDeleteItemSvcParams) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for SvcbResourceRecordDeleteItemSvcParams {
        fn default() -> Self {
            SvcbResourceRecordDeleteItemSvcParams("".to_string())
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordDeleteItemSvcParams {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 0usize {
                return Err("shorter than 0 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new(".*").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \".*\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordDeleteItemSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SvcbResourceRecordDeleteItemSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SvcbResourceRecordDeleteItemSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SvcbResourceRecordDeleteItemSvcParams {
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

    ///A fully qualified domain name (FQDN) or a single ".", either the alias
    /// target (for AliasMode) or the alternative endpoint (for ServiceMode).
    /// For AliasMode, a TargetName of "." indicates that the service is not
    /// available or does not exist. For ServiceMode, if TargetName has the
    /// value ".", then the owner name of this record is used as the effective
    /// TargetName.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A fully qualified domain name (FQDN) or a single \".\",
    /// either the alias target (for AliasMode) or the alternative endpoint (for
    /// ServiceMode).\nFor AliasMode, a TargetName of \".\" indicates that the
    /// service is not available or does not exist.\nFor ServiceMode, if
    /// TargetName has the value \".\", then the owner name of this record is
    /// used as the effective TargetName.",
    ///  "examples": [
    ///    "_443._https.www.example.com"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/fqdn"
    ///    }
    ///  ],
    ///  "maxLength": 253
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum SvcbResourceRecordDeleteItemTargetName {
        Variant0(SvcbResourceRecordDeleteItemTargetNameVariant0),
        Variant1(::std::string::String),
    }

    impl ::std::fmt::Display for SvcbResourceRecordDeleteItemTargetName {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                Self::Variant0(x) => x.fmt(f),
                Self::Variant1(x) => x.fmt(f),
            }
        }
    }

    impl ::std::convert::From<SvcbResourceRecordDeleteItemTargetNameVariant0>
        for SvcbResourceRecordDeleteItemTargetName
    {
        fn from(value: SvcbResourceRecordDeleteItemTargetNameVariant0) -> Self {
            Self::Variant0(value)
        }
    }

    ///`SvcbResourceRecordDeleteItemTargetNameVariant0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "maxLength": 253
    ///    },
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "not": {
    ///        "$ref": "#/components/schemas/fqdn"
    ///      }
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
    pub enum SvcbResourceRecordDeleteItemTargetNameVariant0 {
        #[serde(rename = ".")]
        X,
    }

    impl ::std::fmt::Display for SvcbResourceRecordDeleteItemTargetNameVariant0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("."),
            }
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordDeleteItemTargetNameVariant0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "." => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordDeleteItemTargetNameVariant0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for SvcbResourceRecordDeleteItemTargetNameVariant0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for SvcbResourceRecordDeleteItemTargetNameVariant0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`SvcbResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "SVCB"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum SvcbResourceRecordDeleteItemType {
        #[serde(rename = "SVCB")]
        Svcb,
    }

    impl ::std::fmt::Display for SvcbResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Svcb => f.write_str("SVCB"),
            }
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "SVCB" => Ok(Self::Svcb),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SvcbResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SvcbResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Specifies the port number for which the SVCB record is applicable.
    ///If specified, it must be a single wildcard (an asterisk symbol) or a
    /// string that starts with an underscore and continues with a number from 1
    /// to 65535.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the port number for which the SVCB record is
    /// applicable.\nIf specified, it must be a single wildcard (an asterisk
    /// symbol) or a string that starts with an underscore and continues with a
    /// number from 1 to 65535.",
    ///  "examples": [
    ///    "_443"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "*"
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/underscoredPort"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct SvcbResourceRecordPort {
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_0: ::std::option::Option<SvcbResourceRecordPortSubtype0>,
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_1: ::std::option::Option<UnderscoredPort>,
    }

    impl ::std::default::Default for SvcbResourceRecordPort {
        fn default() -> Self {
            Self {
                subtype_0: Default::default(),
                subtype_1: Default::default(),
            }
        }
    }

    ///`SvcbResourceRecordPortSubtype0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "*"
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
    pub enum SvcbResourceRecordPortSubtype0 {
        #[serde(rename = "*")]
        X,
    }

    impl ::std::fmt::Display for SvcbResourceRecordPortSubtype0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("*"),
            }
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordPortSubtype0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "*" => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SvcbResourceRecordPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SvcbResourceRecordPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Indicates the scheme over which the SVCB record applies, such as "_tcp"
    /// for TCP or "_udp" for UDP.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Indicates the scheme over which the SVCB record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///  "examples": [
    ///    "_tcp"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SvcbResourceRecordScheme(::std::string::String);
    impl ::std::ops::Deref for SvcbResourceRecordScheme {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SvcbResourceRecordScheme> for ::std::string::String {
        fn from(value: SvcbResourceRecordScheme) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordScheme {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordScheme {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SvcbResourceRecordScheme {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SvcbResourceRecordScheme {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SvcbResourceRecordScheme {
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

    ///A whitespace-separated list with parameters describing the alternative
    /// endpoint at TargetName (only used in ServiceMode and otherwise ignored).
    /// Each SvcParam consisting of a SvcParamKey=SvcParamValue pair or a
    /// standalone SvcParamKey.
    ///
    ///Initial keys: "mandatory", "alpn", "no-default-alpn", "port",
    /// "ipv4hint", "ech", "ipv6hint", "dohpath", "ohttp",
    /// "tls-supported-groups".
    ///
    ///Arbitrary keys can be represented using the unknown-key presentation
    /// format "keyNNNNN" where NNNNN is the numeric value of the key type
    /// without leading zeros (Number 0-65535).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A whitespace-separated list with parameters describing
    /// the alternative endpoint at TargetName (only used in ServiceMode and
    /// otherwise ignored).\nEach SvcParam consisting of a
    /// SvcParamKey=SvcParamValue pair or a standalone SvcParamKey.\n\nInitial
    /// keys: \"mandatory\", \"alpn\", \"no-default-alpn\", \"port\",
    /// \"ipv4hint\", \"ech\", \"ipv6hint\", \"dohpath\", \"ohttp\",
    /// \"tls-supported-groups\".\n\nArbitrary keys can be represented using the
    /// unknown-key presentation format \"keyNNNNN\" where NNNNN is the numeric
    /// value of the key type without leading zeros (Number 0-65535).",
    ///  "default": "",
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 0,
    ///  "pattern": ".*"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SvcbResourceRecordSvcParams(::std::string::String);
    impl ::std::ops::Deref for SvcbResourceRecordSvcParams {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<SvcbResourceRecordSvcParams> for ::std::string::String {
        fn from(value: SvcbResourceRecordSvcParams) -> Self {
            value.0
        }
    }

    impl ::std::default::Default for SvcbResourceRecordSvcParams {
        fn default() -> Self {
            SvcbResourceRecordSvcParams("".to_string())
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordSvcParams {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 0usize {
                return Err("shorter than 0 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new(".*").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \".*\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SvcbResourceRecordSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SvcbResourceRecordSvcParams {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for SvcbResourceRecordSvcParams {
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

    ///A fully qualified domain name (FQDN) or a single ".", either the alias
    /// target (for AliasMode) or the alternative endpoint (for ServiceMode).
    /// For AliasMode, a TargetName of "." indicates that the service is not
    /// available or does not exist. For ServiceMode, if TargetName has the
    /// value ".", then the owner name of this record is used as the effective
    /// TargetName.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A fully qualified domain name (FQDN) or a single \".\",
    /// either the alias target (for AliasMode) or the alternative endpoint (for
    /// ServiceMode).\nFor AliasMode, a TargetName of \".\" indicates that the
    /// service is not available or does not exist.\nFor ServiceMode, if
    /// TargetName has the value \".\", then the owner name of this record is
    /// used as the effective TargetName.",
    ///  "examples": [
    ///    "_443._https.www.example.com"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/fqdn"
    ///    }
    ///  ],
    ///  "maxLength": 253
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum SvcbResourceRecordTargetName {
        Variant0(SvcbResourceRecordTargetNameVariant0),
        Variant1(::std::string::String),
    }

    impl ::std::fmt::Display for SvcbResourceRecordTargetName {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                Self::Variant0(x) => x.fmt(f),
                Self::Variant1(x) => x.fmt(f),
            }
        }
    }

    impl ::std::convert::From<SvcbResourceRecordTargetNameVariant0> for SvcbResourceRecordTargetName {
        fn from(value: SvcbResourceRecordTargetNameVariant0) -> Self {
            Self::Variant0(value)
        }
    }

    ///`SvcbResourceRecordTargetNameVariant0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "allOf": [
    ///    {
    ///      "maxLength": 253
    ///    },
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "."
    ///      ]
    ///    },
    ///    {
    ///      "not": {
    ///        "$ref": "#/components/schemas/fqdn"
    ///      }
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
    pub enum SvcbResourceRecordTargetNameVariant0 {
        #[serde(rename = ".")]
        X,
    }

    impl ::std::fmt::Display for SvcbResourceRecordTargetNameVariant0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("."),
            }
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordTargetNameVariant0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "." => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordTargetNameVariant0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SvcbResourceRecordTargetNameVariant0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SvcbResourceRecordTargetNameVariant0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`SvcbResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "SVCB"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum SvcbResourceRecordType {
        #[serde(rename = "SVCB")]
        Svcb,
    }

    impl ::std::fmt::Display for SvcbResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Svcb => f.write_str("SVCB"),
            }
        }
    }

    impl ::std::str::FromStr for SvcbResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "SVCB" => Ok(Self::Svcb),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for SvcbResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for SvcbResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for SvcbResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to associate a TLS server certificate or public key with the domain
    /// name where the record is found.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to associate a TLS server certificate or public
    /// key with the domain name where the record is found.",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "associationData",
    ///    "matching",
    ///    "port",
    ///    "protocol",
    ///    "selector",
    ///    "type",
    ///    "usage"
    ///  ],
    ///  "properties": {
    ///    "associationData": {
    ///      "description": "The actual data (hash or full certificate) that the
    /// TLSA record is associating with the domain name.",
    ///      "examples": [
    ///        "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 64,
    ///      "pattern": "^(?!\\s)(\\s?[0-9a-f]{2})+$"
    ///    },
    ///    "matching": {
    ///      "description": "Defines how the certificate association is
    /// presented in the record",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 255.0,
    ///      "minimum": 0.0
    ///    },
    ///    "port": {
    ///      "description": "Specifies the port number for which the TLSA record
    /// is applicable.\nShould be equal asterisk or must start with an
    /// underscore and have a number between 1 and 65535",
    ///      "examples": [
    ///        "_443"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "*"
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/underscoredPort"
    ///        }
    ///      ]
    ///    },
    ///    "protocol": {
    ///      "description": "Indicates the protocol over which the TLSA record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///      "examples": [
    ///        "_tcp"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "minLength": 2,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "selector": {
    ///      "description": "Specifies which part of the certificate to use",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 255.0,
    ///      "minimum": 0.0
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "TLSA"
    ///      ]
    ///    },
    ///    "usage": {
    ///      "description": "Specifies how the certificate association is used",
    ///      "examples": [
    ///        2
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 255.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TlsaResourceRecord {
        ///The actual data (hash or full certificate) that the TLSA record is
        /// associating with the domain name.
        #[serde(rename = "associationData")]
        pub association_data: TlsaResourceRecordAssociationData,
        pub group: ResourceRecordsGroup,
        ///Defines how the certificate association is presented in the record
        pub matching: u16,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the port number for which the TLSA record is applicable.
        ///Should be equal asterisk or must start with an underscore and have a
        /// number between 1 and 65535
        pub port: TlsaResourceRecordPort,
        ///Indicates the protocol over which the TLSA record applies, such as
        /// "_tcp" for TCP or "_udp" for UDP.
        pub protocol: TlsaResourceRecordProtocol,
        ///Specifies which part of the certificate to use
        pub selector: u16,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: TlsaResourceRecordType,
        ///Specifies how the certificate association is used
        pub usage: u16,
    }

    ///The actual data (hash or full certificate) that the TLSA record is
    /// associating with the domain name.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The actual data (hash or full certificate) that the
    /// TLSA record is associating with the domain name.",
    ///  "examples": [
    ///    "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 64,
    ///  "pattern": "^(?!\\s)(\\s?[0-9a-f]{2})+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TlsaResourceRecordAssociationData(::std::string::String);
    impl ::std::ops::Deref for TlsaResourceRecordAssociationData {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TlsaResourceRecordAssociationData> for ::std::string::String {
        fn from(value: TlsaResourceRecordAssociationData) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordAssociationData {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 64usize {
                return Err("shorter than 64 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^(?!\\s)(\\s?[0-9a-f]{2})+$").unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^(?!\\s)(\\s?[0-9a-f]{2})+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordAssociationData {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TlsaResourceRecordAssociationData {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TlsaResourceRecordAssociationData {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TlsaResourceRecordAssociationData {
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

    ///Used to associate a TLS server certificate or public key with the domain
    /// name where the record is found.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to associate a TLS server certificate or public
    /// key with the domain name where the record is found.",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "associationData",
    ///    "matching",
    ///    "port",
    ///    "protocol",
    ///    "selector",
    ///    "type",
    ///    "usage"
    ///  ],
    ///  "properties": {
    ///    "associationData": {
    ///      "description": "The actual data (hash or full certificate) that the
    /// TLSA record is associating with the domain name.",
    ///      "examples": [
    ///        "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 64,
    ///      "pattern": "^(?!\\s)(\\s?[0-9a-f]{2})+$"
    ///    },
    ///    "matching": {
    ///      "description": "Defines how the certificate association is
    /// presented in the record",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 255.0,
    ///      "minimum": 0.0
    ///    },
    ///    "port": {
    ///      "description": "Specifies the port number for which the TLSA record
    /// is applicable.\nShould be equal asterisk or must start with an
    /// underscore and have a number between 1 and 65535",
    ///      "examples": [
    ///        "_443"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "*"
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/underscoredPort"
    ///        }
    ///      ]
    ///    },
    ///    "protocol": {
    ///      "description": "Indicates the protocol over which the TLSA record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///      "examples": [
    ///        "_tcp"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "minLength": 2,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "selector": {
    ///      "description": "Specifies which part of the certificate to use",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 255.0,
    ///      "minimum": 0.0
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "TLSA"
    ///      ]
    ///    },
    ///    "usage": {
    ///      "description": "Specifies how the certificate association is used",
    ///      "examples": [
    ///        2
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 255.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TlsaResourceRecordCreateOrUpdateItem {
        ///The actual data (hash or full certificate) that the TLSA record is
        /// associating with the domain name.
        #[serde(rename = "associationData")]
        pub association_data: TlsaResourceRecordCreateOrUpdateItemAssociationData,
        ///Defines how the certificate association is presented in the record
        pub matching: u16,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the port number for which the TLSA record is applicable.
        ///Should be equal asterisk or must start with an underscore and have a
        /// number between 1 and 65535
        pub port: TlsaResourceRecordCreateOrUpdateItemPort,
        ///Indicates the protocol over which the TLSA record applies, such as
        /// "_tcp" for TCP or "_udp" for UDP.
        pub protocol: TlsaResourceRecordCreateOrUpdateItemProtocol,
        ///Specifies which part of the certificate to use
        pub selector: u16,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: TlsaResourceRecordCreateOrUpdateItemType,
        ///Specifies how the certificate association is used
        pub usage: u16,
    }

    ///The actual data (hash or full certificate) that the TLSA record is
    /// associating with the domain name.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The actual data (hash or full certificate) that the
    /// TLSA record is associating with the domain name.",
    ///  "examples": [
    ///    "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 64,
    ///  "pattern": "^(?!\\s)(\\s?[0-9a-f]{2})+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TlsaResourceRecordCreateOrUpdateItemAssociationData(::std::string::String);
    impl ::std::ops::Deref for TlsaResourceRecordCreateOrUpdateItemAssociationData {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TlsaResourceRecordCreateOrUpdateItemAssociationData>
        for ::std::string::String
    {
        fn from(value: TlsaResourceRecordCreateOrUpdateItemAssociationData) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordCreateOrUpdateItemAssociationData {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 64usize {
                return Err("shorter than 64 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^(?!\\s)(\\s?[0-9a-f]{2})+$").unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^(?!\\s)(\\s?[0-9a-f]{2})+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordCreateOrUpdateItemAssociationData {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for TlsaResourceRecordCreateOrUpdateItemAssociationData
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for TlsaResourceRecordCreateOrUpdateItemAssociationData
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TlsaResourceRecordCreateOrUpdateItemAssociationData {
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

    ///Specifies the port number for which the TLSA record is applicable.
    ///Should be equal asterisk or must start with an underscore and have a
    /// number between 1 and 65535
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the port number for which the TLSA record is
    /// applicable.\nShould be equal asterisk or must start with an underscore
    /// and have a number between 1 and 65535",
    ///  "examples": [
    ///    "_443"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "*"
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/underscoredPort"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TlsaResourceRecordCreateOrUpdateItemPort {
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_0: ::std::option::Option<TlsaResourceRecordCreateOrUpdateItemPortSubtype0>,
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_1: ::std::option::Option<UnderscoredPort>,
    }

    impl ::std::default::Default for TlsaResourceRecordCreateOrUpdateItemPort {
        fn default() -> Self {
            Self {
                subtype_0: Default::default(),
                subtype_1: Default::default(),
            }
        }
    }

    ///`TlsaResourceRecordCreateOrUpdateItemPortSubtype0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "*"
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
    pub enum TlsaResourceRecordCreateOrUpdateItemPortSubtype0 {
        #[serde(rename = "*")]
        X,
    }

    impl ::std::fmt::Display for TlsaResourceRecordCreateOrUpdateItemPortSubtype0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("*"),
            }
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordCreateOrUpdateItemPortSubtype0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "*" => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordCreateOrUpdateItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for TlsaResourceRecordCreateOrUpdateItemPortSubtype0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for TlsaResourceRecordCreateOrUpdateItemPortSubtype0
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Indicates the protocol over which the TLSA record applies, such as
    /// "_tcp" for TCP or "_udp" for UDP.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Indicates the protocol over which the TLSA record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///  "examples": [
    ///    "_tcp"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "minLength": 2,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TlsaResourceRecordCreateOrUpdateItemProtocol(::std::string::String);
    impl ::std::ops::Deref for TlsaResourceRecordCreateOrUpdateItemProtocol {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TlsaResourceRecordCreateOrUpdateItemProtocol> for ::std::string::String {
        fn from(value: TlsaResourceRecordCreateOrUpdateItemProtocol) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordCreateOrUpdateItemProtocol {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordCreateOrUpdateItemProtocol {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for TlsaResourceRecordCreateOrUpdateItemProtocol
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for TlsaResourceRecordCreateOrUpdateItemProtocol
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TlsaResourceRecordCreateOrUpdateItemProtocol {
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

    ///`TlsaResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "TLSA"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum TlsaResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "TLSA")]
        Tlsa,
    }

    impl ::std::fmt::Display for TlsaResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Tlsa => f.write_str("TLSA"),
            }
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "TLSA" => Ok(Self::Tlsa),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TlsaResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TlsaResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Used to associate a TLS server certificate or public key with the domain
    /// name where the record is found.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Used to associate a TLS server certificate or public
    /// key with the domain name where the record is found.",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "associationData",
    ///    "matching",
    ///    "port",
    ///    "protocol",
    ///    "selector",
    ///    "type",
    ///    "usage"
    ///  ],
    ///  "properties": {
    ///    "associationData": {
    ///      "description": "The actual data (hash or full certificate) that the
    /// TLSA record is associating with the domain name.",
    ///      "examples": [
    ///        "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 64,
    ///      "pattern": "^(?!\\s)(\\s?[0-9a-f]{2})+$"
    ///    },
    ///    "matching": {
    ///      "description": "Defines how the certificate association is
    /// presented in the record",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 255.0,
    ///      "minimum": 0.0
    ///    },
    ///    "port": {
    ///      "description": "Specifies the port number for which the TLSA record
    /// is applicable.\nShould be equal asterisk or must start with an
    /// underscore and have a number between 1 and 65535",
    ///      "examples": [
    ///        "_443"
    ///      ],
    ///      "anyOf": [
    ///        {
    ///          "type": "string",
    ///          "enum": [
    ///            "*"
    ///          ]
    ///        },
    ///        {
    ///          "$ref": "#/components/schemas/underscoredPort"
    ///        }
    ///      ]
    ///    },
    ///    "protocol": {
    ///      "description": "Indicates the protocol over which the TLSA record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///      "examples": [
    ///        "_tcp"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 63,
    ///      "minLength": 2,
    ///      "pattern": "_[a-zA-Z0-9-]+"
    ///    },
    ///    "selector": {
    ///      "description": "Specifies which part of the certificate to use",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 255.0,
    ///      "minimum": 0.0
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "TLSA"
    ///      ]
    ///    },
    ///    "usage": {
    ///      "description": "Specifies how the certificate association is used",
    ///      "examples": [
    ///        2
    ///      ],
    ///      "type": "integer",
    ///      "format": "uint16",
    ///      "maximum": 255.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TlsaResourceRecordDeleteItem {
        ///The actual data (hash or full certificate) that the TLSA record is
        /// associating with the domain name.
        #[serde(rename = "associationData")]
        pub association_data: TlsaResourceRecordDeleteItemAssociationData,
        ///Defines how the certificate association is presented in the record
        pub matching: u16,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the port number for which the TLSA record is applicable.
        ///Should be equal asterisk or must start with an underscore and have a
        /// number between 1 and 65535
        pub port: TlsaResourceRecordDeleteItemPort,
        ///Indicates the protocol over which the TLSA record applies, such as
        /// "_tcp" for TCP or "_udp" for UDP.
        pub protocol: TlsaResourceRecordDeleteItemProtocol,
        ///Specifies which part of the certificate to use
        pub selector: u16,
        #[serde(rename = "type")]
        pub type_: TlsaResourceRecordDeleteItemType,
        ///Specifies how the certificate association is used
        pub usage: u16,
    }

    ///The actual data (hash or full certificate) that the TLSA record is
    /// associating with the domain name.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The actual data (hash or full certificate) that the
    /// TLSA record is associating with the domain name.",
    ///  "examples": [
    ///    "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 64,
    ///  "pattern": "^(?!\\s)(\\s?[0-9a-f]{2})+$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TlsaResourceRecordDeleteItemAssociationData(::std::string::String);
    impl ::std::ops::Deref for TlsaResourceRecordDeleteItemAssociationData {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TlsaResourceRecordDeleteItemAssociationData> for ::std::string::String {
        fn from(value: TlsaResourceRecordDeleteItemAssociationData) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordDeleteItemAssociationData {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 64usize {
                return Err("shorter than 64 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^(?!\\s)(\\s?[0-9a-f]{2})+$").unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^(?!\\s)(\\s?[0-9a-f]{2})+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordDeleteItemAssociationData {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String>
        for TlsaResourceRecordDeleteItemAssociationData
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String>
        for TlsaResourceRecordDeleteItemAssociationData
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TlsaResourceRecordDeleteItemAssociationData {
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

    ///Specifies the port number for which the TLSA record is applicable.
    ///Should be equal asterisk or must start with an underscore and have a
    /// number between 1 and 65535
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the port number for which the TLSA record is
    /// applicable.\nShould be equal asterisk or must start with an underscore
    /// and have a number between 1 and 65535",
    ///  "examples": [
    ///    "_443"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "*"
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/underscoredPort"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TlsaResourceRecordDeleteItemPort {
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_0: ::std::option::Option<TlsaResourceRecordDeleteItemPortSubtype0>,
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_1: ::std::option::Option<UnderscoredPort>,
    }

    impl ::std::default::Default for TlsaResourceRecordDeleteItemPort {
        fn default() -> Self {
            Self {
                subtype_0: Default::default(),
                subtype_1: Default::default(),
            }
        }
    }

    ///`TlsaResourceRecordDeleteItemPortSubtype0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "*"
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
    pub enum TlsaResourceRecordDeleteItemPortSubtype0 {
        #[serde(rename = "*")]
        X,
    }

    impl ::std::fmt::Display for TlsaResourceRecordDeleteItemPortSubtype0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("*"),
            }
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordDeleteItemPortSubtype0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "*" => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordDeleteItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TlsaResourceRecordDeleteItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TlsaResourceRecordDeleteItemPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Indicates the protocol over which the TLSA record applies, such as
    /// "_tcp" for TCP or "_udp" for UDP.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Indicates the protocol over which the TLSA record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///  "examples": [
    ///    "_tcp"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "minLength": 2,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TlsaResourceRecordDeleteItemProtocol(::std::string::String);
    impl ::std::ops::Deref for TlsaResourceRecordDeleteItemProtocol {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TlsaResourceRecordDeleteItemProtocol> for ::std::string::String {
        fn from(value: TlsaResourceRecordDeleteItemProtocol) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordDeleteItemProtocol {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordDeleteItemProtocol {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TlsaResourceRecordDeleteItemProtocol {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TlsaResourceRecordDeleteItemProtocol {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TlsaResourceRecordDeleteItemProtocol {
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

    ///`TlsaResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "TLSA"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum TlsaResourceRecordDeleteItemType {
        #[serde(rename = "TLSA")]
        Tlsa,
    }

    impl ::std::fmt::Display for TlsaResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Tlsa => f.write_str("TLSA"),
            }
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "TLSA" => Ok(Self::Tlsa),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TlsaResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TlsaResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Specifies the port number for which the TLSA record is applicable.
    ///Should be equal asterisk or must start with an underscore and have a
    /// number between 1 and 65535
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Specifies the port number for which the TLSA record is
    /// applicable.\nShould be equal asterisk or must start with an underscore
    /// and have a number between 1 and 65535",
    ///  "examples": [
    ///    "_443"
    ///  ],
    ///  "anyOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "*"
    ///      ]
    ///    },
    ///    {
    ///      "$ref": "#/components/schemas/underscoredPort"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TlsaResourceRecordPort {
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_0: ::std::option::Option<TlsaResourceRecordPortSubtype0>,
        #[serde(
            flatten,
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub subtype_1: ::std::option::Option<UnderscoredPort>,
    }

    impl ::std::default::Default for TlsaResourceRecordPort {
        fn default() -> Self {
            Self {
                subtype_0: Default::default(),
                subtype_1: Default::default(),
            }
        }
    }

    ///`TlsaResourceRecordPortSubtype0`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "*"
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
    pub enum TlsaResourceRecordPortSubtype0 {
        #[serde(rename = "*")]
        X,
    }

    impl ::std::fmt::Display for TlsaResourceRecordPortSubtype0 {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X => f.write_str("*"),
            }
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordPortSubtype0 {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "*" => Ok(Self::X),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TlsaResourceRecordPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TlsaResourceRecordPortSubtype0 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Indicates the protocol over which the TLSA record applies, such as
    /// "_tcp" for TCP or "_udp" for UDP.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Indicates the protocol over which the TLSA record
    /// applies, such as \"_tcp\" for TCP or \"_udp\" for UDP.",
    ///  "examples": [
    ///    "_tcp"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 63,
    ///  "minLength": 2,
    ///  "pattern": "_[a-zA-Z0-9-]+"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TlsaResourceRecordProtocol(::std::string::String);
    impl ::std::ops::Deref for TlsaResourceRecordProtocol {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TlsaResourceRecordProtocol> for ::std::string::String {
        fn from(value: TlsaResourceRecordProtocol) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordProtocol {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 63usize {
                return Err("longer than 63 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("_[a-zA-Z0-9-]+").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"_[a-zA-Z0-9-]+\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordProtocol {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TlsaResourceRecordProtocol {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TlsaResourceRecordProtocol {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TlsaResourceRecordProtocol {
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

    ///`TlsaResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "TLSA"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum TlsaResourceRecordType {
        #[serde(rename = "TLSA")]
        Tlsa,
    }

    impl ::std::fmt::Display for TlsaResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Tlsa => f.write_str("TLSA"),
            }
        }
    }

    impl ::std::str::FromStr for TlsaResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "TLSA" => Ok(Self::Tlsa),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TlsaResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TlsaResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TlsaResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///used to store arbitrary text data associated with a domain.
    ///It is commonly used for various purposes such as verifying domain
    /// ownership for services like email authentication (SPF, DKIM),
    /// providing human-readable information, or storing any text-based
    /// information required by applications
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "used to store arbitrary text data associated with a
    /// domain.\nIt is commonly used for various purposes such as verifying
    /// domain ownership for services\nlike email authentication (SPF, DKIM),
    /// providing human-readable information,\nor storing any text-based
    /// information required by applications",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecord"
    ///    }
    ///  ],
    ///  "required": [
    ///    "type",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "TXT"
    ///      ]
    ///    },
    ///    "value": {
    ///      "description": "Text value",
    ///      "examples": [
    ///        "v=spf1 a mx -all"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 1,
    ///      "pattern": ".*"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TxtResourceRecord {
        pub group: ResourceRecordsGroup,
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: TxtResourceRecordType,
        ///Text value
        pub value: TxtResourceRecordValue,
    }

    ///used to store arbitrary text data associated with a domain.
    ///It is commonly used for various purposes such as verifying domain
    /// ownership for services like email authentication (SPF, DKIM),
    /// providing human-readable information, or storing any text-based
    /// information required by applications
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "used to store arbitrary text data associated with a
    /// domain.\nIt is commonly used for various purposes such as verifying
    /// domain ownership for services\nlike email authentication (SPF, DKIM),
    /// providing human-readable information,\nor storing any text-based
    /// information required by applications",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordCreateOrUpdateItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "type",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "TXT"
    ///      ]
    ///    },
    ///    "value": {
    ///      "description": "Text value",
    ///      "examples": [
    ///        "v=spf1 a mx -all"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 1,
    ///      "pattern": ".*"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TxtResourceRecordCreateOrUpdateItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        ///Specifies the amount of time in seconds that a DNS record should be
        /// cached by a resolver or a caching server before it expires
        /// and needs to be refreshed from the authoritative DNS servers
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ttl: ::std::option::Option<i32>,
        #[serde(rename = "type")]
        pub type_: TxtResourceRecordCreateOrUpdateItemType,
        ///Text value
        pub value: TxtResourceRecordCreateOrUpdateItemValue,
    }

    ///`TxtResourceRecordCreateOrUpdateItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "TXT"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum TxtResourceRecordCreateOrUpdateItemType {
        #[serde(rename = "TXT")]
        Txt,
    }

    impl ::std::fmt::Display for TxtResourceRecordCreateOrUpdateItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Txt => f.write_str("TXT"),
            }
        }
    }

    impl ::std::str::FromStr for TxtResourceRecordCreateOrUpdateItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "TXT" => Ok(Self::Txt),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TxtResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TxtResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TxtResourceRecordCreateOrUpdateItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Text value
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Text value",
    ///  "examples": [
    ///    "v=spf1 a mx -all"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 1,
    ///  "pattern": ".*"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TxtResourceRecordCreateOrUpdateItemValue(::std::string::String);
    impl ::std::ops::Deref for TxtResourceRecordCreateOrUpdateItemValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TxtResourceRecordCreateOrUpdateItemValue> for ::std::string::String {
        fn from(value: TxtResourceRecordCreateOrUpdateItemValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TxtResourceRecordCreateOrUpdateItemValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new(".*").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \".*\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TxtResourceRecordCreateOrUpdateItemValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TxtResourceRecordCreateOrUpdateItemValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TxtResourceRecordCreateOrUpdateItemValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TxtResourceRecordCreateOrUpdateItemValue {
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

    ///used to store arbitrary text data associated with a domain.
    ///It is commonly used for various purposes such as verifying domain
    /// ownership for services like email authentication (SPF, DKIM),
    /// providing human-readable information, or storing any text-based
    /// information required by applications
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "used to store arbitrary text data associated with a
    /// domain.\nIt is commonly used for various purposes such as verifying
    /// domain ownership for services\nlike email authentication (SPF, DKIM),
    /// providing human-readable information,\nor storing any text-based
    /// information required by applications",
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/ResourceRecordDeleteItem"
    ///    }
    ///  ],
    ///  "required": [
    ///    "type",
    ///    "value"
    ///  ],
    ///  "properties": {
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "TXT"
    ///      ]
    ///    },
    ///    "value": {
    ///      "description": "Text value",
    ///      "examples": [
    ///        "v=spf1 a mx -all"
    ///      ],
    ///      "type": "string",
    ///      "maxLength": 65535,
    ///      "minLength": 1,
    ///      "pattern": ".*"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct TxtResourceRecordDeleteItem {
        ///Name of resource record excluding domain name part. '@' can be used
        /// as an apex domain
        pub name: HostNameValue,
        #[serde(rename = "type")]
        pub type_: TxtResourceRecordDeleteItemType,
        ///Text value
        pub value: TxtResourceRecordDeleteItemValue,
    }

    ///`TxtResourceRecordDeleteItemType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "TXT"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum TxtResourceRecordDeleteItemType {
        #[serde(rename = "TXT")]
        Txt,
    }

    impl ::std::fmt::Display for TxtResourceRecordDeleteItemType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Txt => f.write_str("TXT"),
            }
        }
    }

    impl ::std::str::FromStr for TxtResourceRecordDeleteItemType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "TXT" => Ok(Self::Txt),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TxtResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TxtResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TxtResourceRecordDeleteItemType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Text value
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Text value",
    ///  "examples": [
    ///    "v=spf1 a mx -all"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 1,
    ///  "pattern": ".*"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TxtResourceRecordDeleteItemValue(::std::string::String);
    impl ::std::ops::Deref for TxtResourceRecordDeleteItemValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TxtResourceRecordDeleteItemValue> for ::std::string::String {
        fn from(value: TxtResourceRecordDeleteItemValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TxtResourceRecordDeleteItemValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new(".*").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \".*\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TxtResourceRecordDeleteItemValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TxtResourceRecordDeleteItemValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TxtResourceRecordDeleteItemValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TxtResourceRecordDeleteItemValue {
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

    ///`TxtResourceRecordType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "TXT"
    ///  ],
    ///  "maxLength": 5,
    ///  "minLength": 1,
    ///  "pattern": "\\w+"
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
    pub enum TxtResourceRecordType {
        #[serde(rename = "TXT")]
        Txt,
    }

    impl ::std::fmt::Display for TxtResourceRecordType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Txt => f.write_str("TXT"),
            }
        }
    }

    impl ::std::str::FromStr for TxtResourceRecordType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "TXT" => Ok(Self::Txt),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for TxtResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TxtResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TxtResourceRecordType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///Text value
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Text value",
    ///  "examples": [
    ///    "v=spf1 a mx -all"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 65535,
    ///  "minLength": 1,
    ///  "pattern": ".*"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct TxtResourceRecordValue(::std::string::String);
    impl ::std::ops::Deref for TxtResourceRecordValue {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<TxtResourceRecordValue> for ::std::string::String {
        fn from(value: TxtResourceRecordValue) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for TxtResourceRecordValue {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 65535usize {
                return Err("longer than 65535 characters".into());
            }
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new(".*").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \".*\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for TxtResourceRecordValue {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for TxtResourceRecordValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for TxtResourceRecordValue {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for TxtResourceRecordValue {
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

    ///`UnauthorizedError`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "detail"
    ///  ],
    ///  "properties": {
    ///    "detail": {
    ///      "description": "A general message about the exception",
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "pattern": "^[\\s|\\S]*$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct UnauthorizedError {
        ///A general message about the exception
        pub detail: UnauthorizedErrorDetail,
    }

    ///A general message about the exception
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A general message about the exception",
    ///  "readOnly": true,
    ///  "type": "string",
    ///  "pattern": "^[\\s|\\S]*$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UnauthorizedErrorDetail(::std::string::String);
    impl ::std::ops::Deref for UnauthorizedErrorDetail {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UnauthorizedErrorDetail> for ::std::string::String {
        fn from(value: UnauthorizedErrorDetail) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UnauthorizedErrorDetail {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[\\s|\\S]*$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\s|\\S]*$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UnauthorizedErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UnauthorizedErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UnauthorizedErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UnauthorizedErrorDetail {
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

    ///A string that starts with an underscore and continues with a port number
    /// from 1 to 65535
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A string that starts with an underscore and continues
    /// with a port number from 1 to 65535",
    ///  "examples": [
    ///    "_8443"
    ///  ],
    ///  "type": "string",
    ///  "maxLength": 6,
    ///  "minLength": 2,
    ///  "pattern": "^_(6[0-5]{2}[0-3][0-5]|[1-5]?([0-9]){2,4}|[1-9]?)$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UnderscoredPort(::std::string::String);
    impl ::std::ops::Deref for UnderscoredPort {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UnderscoredPort> for ::std::string::String {
        fn from(value: UnderscoredPort) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UnderscoredPort {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 6usize {
                return Err("longer than 6 characters".into());
            }
            if value.chars().count() < 2usize {
                return Err("shorter than 2 characters".into());
            }
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regress::Regex::new("^_(6[0-5]{2}[0-3][0-5]|[1-5]?([0-9]){2,4}|[1-9]?)$")
                        .unwrap()
                });
            if PATTERN.find(value).is_none() {
                return Err(
                    "doesn't match pattern \"^_(6[0-5]{2}[0-3][0-5]|[1-5]?([0-9]){2,4}|[1-9]?)$\""
                        .into(),
                );
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UnderscoredPort {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UnderscoredPort {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UnderscoredPort {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UnderscoredPort {
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

    ///`UnexpectedError`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "detail"
    ///  ],
    ///  "properties": {
    ///    "detail": {
    ///      "description": "A general message about the exception",
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "pattern": "^[\\s|\\S]*$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct UnexpectedError {
        ///A general message about the exception
        pub detail: UnexpectedErrorDetail,
    }

    ///A general message about the exception
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A general message about the exception",
    ///  "readOnly": true,
    ///  "type": "string",
    ///  "pattern": "^[\\s|\\S]*$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct UnexpectedErrorDetail(::std::string::String);
    impl ::std::ops::Deref for UnexpectedErrorDetail {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<UnexpectedErrorDetail> for ::std::string::String {
        fn from(value: UnexpectedErrorDetail) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for UnexpectedErrorDetail {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[\\s|\\S]*$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\s|\\S]*$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for UnexpectedErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UnexpectedErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UnexpectedErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for UnexpectedErrorDetail {
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

    ///Update SellerHub domain request
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Update SellerHub domain request",
    ///  "type": "object",
    ///  "properties": {
    ///    "binPrice": {
    ///      "description": "Buy It Now (BIN) price for the domain",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/PriceUpdate"
    ///        }
    ///      ]
    ///    },
    ///    "binPriceEnabled": {
    ///      "description": "Enable or disable the Buy It Now (BIN) option",
    ///      "type": "boolean"
    ///    },
    ///    "description": {
    ///      "description": "Domain description",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/domainDescription"
    ///        }
    ///      ]
    ///    },
    ///    "displayName": {
    ///      "description": "Display name for the domain",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/displayName"
    ///        }
    ///      ]
    ///    },
    ///    "minPrice": {
    ///      "description": "Minimum offer price for the domain",
    ///      "allOf": [
    ///        {
    ///          "$ref": "#/components/schemas/PriceUpdate"
    ///        }
    ///      ]
    ///    },
    ///    "minPriceEnabled": {
    ///      "description": "Enable or disable offer negotiation with minimum
    /// price",
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct UpdateSellerHubDomainRequest {
        ///Buy It Now (BIN) price for the domain
        #[serde(
            rename = "binPrice",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub bin_price: ::std::option::Option<PriceUpdate>,
        ///Enable or disable the Buy It Now (BIN) option
        #[serde(
            rename = "binPriceEnabled",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub bin_price_enabled: ::std::option::Option<bool>,
        ///Domain description
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub description: ::std::option::Option<DomainDescription>,
        ///Display name for the domain
        #[serde(
            rename = "displayName",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub display_name: ::std::option::Option<DisplayName>,
        ///Minimum offer price for the domain
        #[serde(
            rename = "minPrice",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub min_price: ::std::option::Option<PriceUpdate>,
        ///Enable or disable offer negotiation with minimum price
        #[serde(
            rename = "minPriceEnabled",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub min_price_enabled: ::std::option::Option<bool>,
    }

    impl ::std::default::Default for UpdateSellerHubDomainRequest {
        fn default() -> Self {
            Self {
                bin_price: Default::default(),
                bin_price_enabled: Default::default(),
                description: Default::default(),
                display_name: Default::default(),
                min_price: Default::default(),
                min_price_enabled: Default::default(),
            }
        }
    }

    ///`UsAttributeDetails`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "allOf": [
    ///    {
    ///      "$ref": "#/components/schemas/AttributeDetails"
    ///    }
    ///  ],
    ///  "required": [
    ///    "appPurpose",
    ///    "nexusCategory",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "appPurpose": {
    ///      "type": "string",
    ///      "enum": [
    ///        "P1",
    ///        "P2",
    ///        "P3",
    ///        "P4",
    ///        "P5"
    ///      ]
    ///    },
    ///    "nexusCategory": {
    ///      "type": "string",
    ///      "enum": [
    ///        "C11",
    ///        "C12",
    ///        "C21",
    ///        "C31",
    ///        "C32"
    ///      ]
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "us"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct UsAttributeDetails {
        #[serde(rename = "appPurpose")]
        pub app_purpose: UsAttributeDetailsAppPurpose,
        #[serde(rename = "nexusCategory")]
        pub nexus_category: UsAttributeDetailsNexusCategory,
        #[serde(rename = "type")]
        pub type_: UsAttributeDetailsType,
    }

    ///`UsAttributeDetailsAppPurpose`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "P1",
    ///    "P2",
    ///    "P3",
    ///    "P4",
    ///    "P5"
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
    pub enum UsAttributeDetailsAppPurpose {
        P1,
        P2,
        P3,
        P4,
        P5,
    }

    impl ::std::fmt::Display for UsAttributeDetailsAppPurpose {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::P1 => f.write_str("P1"),
                Self::P2 => f.write_str("P2"),
                Self::P3 => f.write_str("P3"),
                Self::P4 => f.write_str("P4"),
                Self::P5 => f.write_str("P5"),
            }
        }
    }

    impl ::std::str::FromStr for UsAttributeDetailsAppPurpose {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "P1" => Ok(Self::P1),
                "P2" => Ok(Self::P2),
                "P3" => Ok(Self::P3),
                "P4" => Ok(Self::P4),
                "P5" => Ok(Self::P5),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for UsAttributeDetailsAppPurpose {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UsAttributeDetailsAppPurpose {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UsAttributeDetailsAppPurpose {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`UsAttributeDetailsNexusCategory`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "C11",
    ///    "C12",
    ///    "C21",
    ///    "C31",
    ///    "C32"
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
    pub enum UsAttributeDetailsNexusCategory {
        C11,
        C12,
        C21,
        C31,
        C32,
    }

    impl ::std::fmt::Display for UsAttributeDetailsNexusCategory {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::C11 => f.write_str("C11"),
                Self::C12 => f.write_str("C12"),
                Self::C21 => f.write_str("C21"),
                Self::C31 => f.write_str("C31"),
                Self::C32 => f.write_str("C32"),
            }
        }
    }

    impl ::std::str::FromStr for UsAttributeDetailsNexusCategory {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "C11" => Ok(Self::C11),
                "C12" => Ok(Self::C12),
                "C21" => Ok(Self::C21),
                "C31" => Ok(Self::C31),
                "C32" => Ok(Self::C32),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for UsAttributeDetailsNexusCategory {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UsAttributeDetailsNexusCategory {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UsAttributeDetailsNexusCategory {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`UsAttributeDetailsType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "us"
    ///  ],
    ///  "maxLength": 32,
    ///  "minLength": 2,
    ///  "pattern": "\\w+"
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
    pub enum UsAttributeDetailsType {
        #[serde(rename = "us")]
        Us,
    }

    impl ::std::fmt::Display for UsAttributeDetailsType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Us => f.write_str("us"),
            }
        }
    }

    impl ::std::str::FromStr for UsAttributeDetailsType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "us" => Ok(Self::Us),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for UsAttributeDetailsType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for UsAttributeDetailsType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for UsAttributeDetailsType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    ///`ValidationError`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "data",
    ///    "detail"
    ///  ],
    ///  "properties": {
    ///    "data": {
    ///      "description": "A detailed list of validation errors",
    ///      "readOnly": true,
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "details",
    ///          "field"
    ///        ],
    ///        "properties": {
    ///          "details": {
    ///            "description": "A specific message about what is wrong with
    /// the field",
    ///            "readOnly": true,
    ///            "examples": [
    ///              "The domain name contains invalid characters"
    ///            ],
    ///            "type": "string",
    ///            "pattern": "^[\\s|\\S]*$"
    ///          },
    ///          "field": {
    ///            "description": "The path to the field that caused the
    /// validation error",
    ///            "readOnly": true,
    ///            "examples": [
    ///              "name"
    ///            ],
    ///            "type": "string",
    ///            "pattern": "^[\\s|\\S]*$"
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "detail": {
    ///      "description": "A general message about the exception",
    ///      "readOnly": true,
    ///      "type": "string",
    ///      "pattern": "^[\\s|\\S]*$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ValidationError {
        ///A detailed list of validation errors
        pub data: ::std::vec::Vec<ValidationErrorDataItem>,
        ///A general message about the exception
        pub detail: ValidationErrorDetail,
    }

    ///`ValidationErrorDataItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "details",
    ///    "field"
    ///  ],
    ///  "properties": {
    ///    "details": {
    ///      "description": "A specific message about what is wrong with the
    /// field",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "The domain name contains invalid characters"
    ///      ],
    ///      "type": "string",
    ///      "pattern": "^[\\s|\\S]*$"
    ///    },
    ///    "field": {
    ///      "description": "The path to the field that caused the validation
    /// error",
    ///      "readOnly": true,
    ///      "examples": [
    ///        "name"
    ///      ],
    ///      "type": "string",
    ///      "pattern": "^[\\s|\\S]*$"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct ValidationErrorDataItem {
        ///A specific message about what is wrong with the field
        pub details: ValidationErrorDataItemDetails,
        ///The path to the field that caused the validation error
        pub field: ValidationErrorDataItemField,
    }

    ///A specific message about what is wrong with the field
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A specific message about what is wrong with the field",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "The domain name contains invalid characters"
    ///  ],
    ///  "type": "string",
    ///  "pattern": "^[\\s|\\S]*$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ValidationErrorDataItemDetails(::std::string::String);
    impl ::std::ops::Deref for ValidationErrorDataItemDetails {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ValidationErrorDataItemDetails> for ::std::string::String {
        fn from(value: ValidationErrorDataItemDetails) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ValidationErrorDataItemDetails {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[\\s|\\S]*$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\s|\\S]*$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ValidationErrorDataItemDetails {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ValidationErrorDataItemDetails {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ValidationErrorDataItemDetails {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ValidationErrorDataItemDetails {
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

    ///The path to the field that caused the validation error
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The path to the field that caused the validation
    /// error",
    ///  "readOnly": true,
    ///  "examples": [
    ///    "name"
    ///  ],
    ///  "type": "string",
    ///  "pattern": "^[\\s|\\S]*$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ValidationErrorDataItemField(::std::string::String);
    impl ::std::ops::Deref for ValidationErrorDataItemField {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ValidationErrorDataItemField> for ::std::string::String {
        fn from(value: ValidationErrorDataItemField) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ValidationErrorDataItemField {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[\\s|\\S]*$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\s|\\S]*$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ValidationErrorDataItemField {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ValidationErrorDataItemField {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ValidationErrorDataItemField {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ValidationErrorDataItemField {
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

    ///A general message about the exception
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A general message about the exception",
    ///  "readOnly": true,
    ///  "type": "string",
    ///  "pattern": "^[\\s|\\S]*$"
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ValidationErrorDetail(::std::string::String);
    impl ::std::ops::Deref for ValidationErrorDetail {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }

    impl ::std::convert::From<ValidationErrorDetail> for ::std::string::String {
        fn from(value: ValidationErrorDetail) -> Self {
            value.0
        }
    }

    impl ::std::str::FromStr for ValidationErrorDetail {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
                ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[\\s|\\S]*$").unwrap());
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[\\s|\\S]*$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }

    impl ::std::convert::TryFrom<&str> for ValidationErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ValidationErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ValidationErrorDetail {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ValidationErrorDetail {
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
}

#[derive(Clone, Debug)]
///Client for Spaceship.com
///
///# Spaceship API
///
///## Authentication
///
///Spaceship API uses a combination of API key and API secret for
/// authentication.
///
///### API key setup
///You can generate your API key and secret in [API Manager](https://www.spaceship.com/application/api-manager/). Use the "New API key" button to set up a new API key and follow the guide to get started.
///After the key setup has been successful, it should appear on the API Manager
/// application page.
///
///### API key usage
///As shown in the example below, API consumers shall pass a valid API key and
/// a corresponding API secret in the X-API-Secret headers. You do not need to
/// encode the API key and the API secret. 
/// ```bash
/// curl -X GET '/api/resource' \
/// -H 'X-Api-Secret: F3brvQluT4s8aDB7PeFBH6qKHfH2xTKTneCjZbq3z2w7rj2vV6n_zhSvvJoQ' \
/// -H 'X-Api-Key: JdIS8QYFMZpVKupJtdc3'
/// ```
///
///## Asynchronous Operations
///
///Some API operations may take an extended period to complete. For these
/// long-running operations, the API returns an HTTP 202 Accepted response with
/// a `spaceship-async-operationid` header containing a unique operation
/// identifier.
///
///### How Async Operations Work
///
/// 1. **Initiate Operation**: When you make a request that requires
///    asynchronous processing (e.g., domain registration, transfer operations),
///    the API immediately returns a 202 response.
///
/// 2. **Receive Operation ID**: The response includes the
///    `spaceship-async-operationid` header with a unique identifier for
///    tracking the operation.
///
/// 3. **Poll for Status**: Use the operation ID to poll the
///    `/v1/async-operations/{operationId}` endpoint to check the current status
///    of your operation.
///
/// 4. **Operation States**: Async operations can have the following statuses:
///   - `pending` - The operation is still in progress
///   - `success` - The operation completed successfully
///   - `failed` - The operation encountered an error and could not complete
///
///### Example Workflow
///
///```bash
/// # Step 1: Initiate an async operation (e.g., domain registration)
/// curl -X POST '/api/v1/domains/example.com' \
/// -H 'X-Api-Secret: YOUR_SECRET' \
/// -H 'X-Api-Key: YOUR_KEY' \
/// -H 'Content-Type: application/json' \
/// -d '{
///  "autoRenew": false,
///  "years": 1,
///  "privacyProtection": {
///    "level": "high",
///    "userConsent": true
///  },
///  "contacts": {
///    "registrant": "CONTACT_ID",
///    "admin": "CONTACT_ID",
///    "tech": "CONTACT_ID",
///    "billing": "CONTACT_ID"
///  }
/// }'
///
/// # Response: 202 Accepted
/// # Headers: spaceship-async-operationid: abc123xyz
///
/// # Step 2: Check operation status
/// curl -X GET '/api/v1/async-operations/abc123xyz' \
/// -H 'X-Api-Secret: YOUR_SECRET' \
/// -H 'X-Api-Key: YOUR_KEY'
///
/// # Response includes:
/// # - status: "pending" | "success" | "failed"
/// # - type: Operation type (e.g., "domains_Create")
/// # - details: Additional information about the operation
/// # - createdAt: Timestamp when operation was created
/// # - modifiedAt: Timestamp of last status update
/// ```
///
///### Permissions
///
/// - <b id="scopes/domains:read">domains:read</b> - Read domain information and
///   check configuration
/// - <b id="scopes/domains:write">domains:write</b> - Manage domains and domain
///   settings
/// - <b id="scopes/domains:transfer">domains:transfer</b> - Transfer domains in
///   and out
/// - <b id="scopes/domains:billing">domains:billing</b> - Manage domains
///   billing operations
/// - <b id="scopes/contacts:write">contacts:write</b> - Save contact details
/// - <b id="scopes/contacts:read">contacts:read</b> - Read contact details
/// - <b id="scopes/dnsrecords:write">dnsrecords:write</b> - Write DNS resource
///   records
/// - <b id="scopes/dnsrecords:read">dnsrecords:read</b> - Read DNS resource
///   records
/// - <b id="scopes/asyncoperations:read">asyncoperations:read</b> - Read async
///   operations details
/// - <b id="scopes/sellerhub:read">sellerhub:read</b> - Read SellerHub Domains
/// - <b id="scopes/sellerhub:write">sellerhub:write</b> - Write SellerHub
///   Domains
///
///Version: 1.0.0
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
        "1.0.0"
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
    ///Obtain async operation details
    ///
    ///Retrieves the details of an async operation, including its status and
    /// any associated details. This operation is essential for tracking the
    /// progress and outcome of async operations, providing users with the
    /// necessary information to manage their requests effectively.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/asyncoperations:read">asyncoperations:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for fetching async operation details is 60 requests per
    ///   user, within 300 seconds.
    ///
    ///Sends a `GET` request to `/v1/async-operations/{operationId}`
    ///
    ///Arguments:
    /// - `operation_id`: Unique ID of async operation
    pub async fn get_async_operation_details<'a>(
        &'a self,
        operation_id: &'a types::GetAsyncOperationDetailsOperationId,
    ) -> Result<ResponseValue<types::AsyncOperationData>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/async-operations/{}",
            self.baseurl,
            encode_path(&operation_id.to_string()),
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
            operation_id: "get_async_operation_details",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Save contact details
    ///
    ///Save details for the contact and return the generated contact ID.
    ///
    /// > Validation rules for some parameters (such as `stateProvince` and
    /// > `postalCode`) depend on the selected country. These fields may be
    /// > required based on the country’s validation logic
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/contacts:write">contacts:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for saving contact details is 300 requests per user, within
    ///   300 seconds.
    ///
    ///Sends a `PUT` request to `/v1/contacts`
    ///
    ///Arguments:
    /// - `body`: Key-value pairs with contact details
    pub async fn save_details<'a>(
        &'a self,
        body: &'a types::ContactDetails,
    ) -> Result<ResponseValue<types::ContactsSaveResponse>, Error<ByteStream>> {
        let url = format!("{}/v1/contacts", self.baseurl,);
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
            operation_id: "save_details",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Save contact attributes
    ///
    ///Save contact attributes and return the generated contact ID.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/contacts:write">contacts:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for saving contact attribute details is 300 requests per
    ///   user, within 300 seconds.
    ///
    ///Sends a `PUT` request to `/v1/contacts/attributes`
    ///
    ///Arguments:
    /// - `body`: Details
    pub async fn save_contact_attributes<'a>(
        &'a self,
        body: &'a types::AttributeDetails,
    ) -> Result<ResponseValue<types::AttributesContactsAttributesResponse>, Error<ByteStream>> {
        let url = format!("{}/v1/contacts/attributes", self.baseurl,);
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
            operation_id: "save_contact_attributes",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Read attribute details
    ///
    ///Read attribute details by contact ID.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/contacts:read">contacts:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for listing contact attribute details is 5 requests per
    ///   attribute, within 300 seconds.
    ///
    ///Sends a `GET` request to `/v1/contacts/attributes/{contact}`
    pub async fn read_attribute_details<'a>(
        &'a self,
        contact: &'a types::ReadAttributeDetailsContact,
    ) -> Result<ResponseValue<types::AttributeDetails>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/contacts/attributes/{}",
            self.baseurl,
            encode_path(&contact.to_string()),
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
            operation_id: "read_attribute_details",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Read contact details
    ///
    ///Read details of the contact by contact ID.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/contacts:read">contacts:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for listing contact details is 5 requests per contact,
    ///   within 300 seconds.
    ///
    ///Sends a `GET` request to `/v1/contacts/{contact}`
    pub async fn read_details<'a>(
        &'a self,
        contact: &'a types::ReadDetailsContact,
    ) -> Result<ResponseValue<types::ContactDetails>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/contacts/{}",
            self.baseurl,
            encode_path(&contact.to_string()),
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
            operation_id: "read_details",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get domain resource records list
    ///
    ///Retrieves a paginated list of resource records, allowing the use of
    /// query parameters to customize the response. This operation is
    /// essential for efficiently managing large collections of resource
    /// records, enabling smooth navigation and retrieval without overloading
    /// the system with unnecessary data.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/dnsrecords:read">dnsrecords:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for getting the domain resource records list is 300 requests
    ///   per user, within 300 seconds.
    ///
    ///Sends a `GET` request to `/v1/dns/records/{domain}`
    ///
    ///Arguments:
    /// - `domain`: The domain whose resource records are being fetched.
    /// - `order_by`: Specifies fields and order to sort the response items
    /// - `skip`: Number of response items to skip
    /// - `take`: Number of response items per page
    pub async fn get_resource_records_list<'a>(
        &'a self,
        domain: &'a str,
        order_by: Option<&'a ::std::vec::Vec<types::GetResourceRecordsListOrderByItem>>,
        skip: i32,
        take: ::std::num::NonZeroU32,
    ) -> Result<ResponseValue<types::GetResourceRecordsListResponse>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/dns/records/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            .query(&progenitor_client::QueryParam::new("orderBy", &order_by))
            .query(&progenitor_client::QueryParam::new("skip", &skip))
            .query(&progenitor_client::QueryParam::new("take", &take))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_resource_records_list",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Save resource records
    ///
    ///Add custom DNS resource records or update TTL.
    ///Records are matched using case-insensitive comparison, except for TXT
    /// records.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/dnsrecords:write">dnsrecords:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for saving resource record details is 300 requests per user,
    ///   per domain, within 300 seconds.
    ///
    ///Sends a `PUT` request to `/v1/dns/records/{domain}`
    ///
    ///Arguments:
    /// - `domain`: The domain whose resource records are being updated.
    /// - `body`
    pub async fn save_records<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::RecordsRecordsUpdateModel,
    ) -> Result<ResponseValue<()>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/dns/records/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "save_records",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            204u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete resource records
    ///
    ///Delete custom DNS resource records.
    ///Records matched using case-insensitive comparison, except for TXT
    /// records, which are case-sensitive.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/dnsrecords:write">dnsrecords:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for deleting resource record details is 300 requests per
    ///   user, per domain, within 300 seconds.
    ///
    ///Sends a `DELETE` request to `/v1/dns/records/{domain}`
    ///
    ///Arguments:
    /// - `domain`: The domain whose resource records are being deleted.
    /// - `body`: Records
    pub async fn delete_records<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::ResourceRecordsListDeleteItem,
    ) -> Result<ResponseValue<()>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/dns/records/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "delete_records",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            204u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get domain list
    ///
    ///Retrieves a paginated list of domains, allowing the use of query
    /// parameters to customize the response. This operation is essential
    /// for efficiently managing large collections of domains, enabling smooth
    /// navigation and retrieval without overloading the system with unnecessary
    /// data.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:read">domains:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for fetching a domain list is 300 requests per user, within
    ///   300 seconds.
    ///
    ///Sends a `GET` request to `/v1/domains`
    ///
    ///Arguments:
    /// - `order_by`: Specifies fields and order to sort the response items
    /// - `skip`: Number of response items to skip
    /// - `take`: Number of response items per page
    pub async fn get_domain_list<'a>(
        &'a self,
        order_by: Option<&'a ::std::vec::Vec<types::GetDomainListOrderByItem>>,
        skip: i32,
        take: ::std::num::NonZeroU32,
    ) -> Result<ResponseValue<types::GetDomainListResponse>, Error<ByteStream>> {
        let url = format!("{}/v1/domains", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("orderBy", &order_by))
            .query(&progenitor_client::QueryParam::new("skip", &skip))
            .query(&progenitor_client::QueryParam::new("take", &take))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_domain_list",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Check domains availability
    ///
    ///Check domains availability.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:read">domains:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit to check domains availability is 30 requests per user,
    ///   within 30 seconds.
    ///
    ///Sends a `POST` request to `/v1/domains/available`
    pub async fn check_domains_availability<'a>(
        &'a self,
        body: &'a types::DomainsGetDomainsAvailabilityRequest,
    ) -> Result<ResponseValue<types::DomainsGetDomainsAvailabilityResult>, Error<ByteStream>> {
        let url = format!("{}/v1/domains/available", self.baseurl,);
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
            operation_id: "check_domains_availability",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get domain info
    ///
    ///Get details of a specific domain.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:read">domains:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for obtaining info for a domain is 5 requests per domain,
    ///   within 300 seconds.
    ///
    ///Sends a `GET` request to `/v1/domains/{domain}`
    ///
    ///Arguments:
    /// - `domain`: Domain name in ASCII format (A-label) whose details are to
    ///   be fetched. The domain name must be provided in a fully qualified
    ///   domain format.
    pub async fn get_domain_info<'a>(
        &'a self,
        domain: &'a str,
    ) -> Result<ResponseValue<types::DomainInfo>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            operation_id: "get_domain_info",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Register the domain
    ///
    ///Register a specific domain.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:billing">domains:billing</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit to register domains is 30 requests per user, within 30
    ///   seconds.
    ///
    ///Sends a `POST` request to `/v1/domains/{domain}`
    ///
    ///Arguments:
    /// - `domain`: The domain name for registration.
    /// - `body`
    pub async fn domain_create<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::DomainCreateRequest,
    ) -> Result<ResponseValue<()>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "domain_create",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            202u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete the domain
    ///
    ///<div style="font-size: 1.8em; color: #f06700; font-weight:bold;
    /// padding-bottom: 20px; line-height: 1.4em"> This API is under
    /// development and currently returns an HTTP 501 status.
    /// <p style="font-size: 0.7em;">The provided information is for preliminary
    /// familiarization only. Once the API is implemented, this notice will be
    /// removed.</p> </div>
    /// Delete the specific domain. Please note a refund will not be provided as
    /// part of this operation.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:write">domains:write</a>
    ///
    ///Sends a `DELETE` request to `/v1/domains/{domain}`
    ///
    ///Arguments:
    /// - `domain`: Domain name in ASCII format (A-label) that must be deleted.
    ///   The domain name must be provided in a fully qualified domain format.
    pub async fn domain_delete<'a>(
        &'a self,
        domain: &'a str,
    ) -> Result<ResponseValue<()>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self.client.delete(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "domain_delete",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            204u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update the domain autorenewal state
    ///
    ///Allows the modification of a domain's autorenewal state.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:write">domains:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for updating the autorenewal state for a domain is 5
    ///   requests per domain, within 300 seconds.
    ///
    ///Sends a `PUT` request to `/v1/domains/{domain}/autorenew`
    ///
    ///Arguments:
    /// - `domain`: The domain whose autorenewal state is being updated.
    /// - `body`: Provides the new autorenewal state for the domain.
    pub async fn update_autorenewal<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::DomainsDomainAutoRenewal,
    ) -> Result<ResponseValue<types::DomainsDomainAutoRenewal>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/autorenew",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            operation_id: "update_autorenewal",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Check single domain availability
    ///
    ///Check single domain availability.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:read">domains:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit to check domain availability is 5 requests per domain,
    ///   within 300 seconds.
    /// * The limit to check domain availability is 30 requests per user, within
    ///   30 seconds.
    ///
    ///Sends a `GET` request to `/v1/domains/{domain}/available`
    ///
    ///Arguments:
    /// - `domain`: Domain name in ASCII format (A-label) whose details are to
    ///   be fetched. The domain name must be provided in a fully qualified
    ///   domain format.
    pub async fn check_single_domain_availability<'a>(
        &'a self,
        domain: &'a str,
    ) -> Result<ResponseValue<types::DomainAvailabilityResult>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/available",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            operation_id: "check_single_domain_availability",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update domain contacts
    ///
    ///Allows the modification of domain name contacts.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:write">domains:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for updating contacts for a domain is 5 requests per domain,
    ///   within 300 seconds.
    ///
    ///Sends a `PUT` request to `/v1/domains/{domain}/contacts`
    ///
    ///Arguments:
    /// - `domain`: The domain whose contacts are being updated.
    /// - `body`: Provides the new set of contacts or the domain.
    pub async fn set_domain_contacts<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::DomainContacts,
    ) -> Result<ResponseValue<types::DomainsPutContactsResponse>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/contacts",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            operation_id: "set_domain_contacts",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update domain nameservers
    ///
    ///Allows the modification of DNS settings for a specific domain by
    /// replacing its current nameservers with new ones. This operation is
    /// crucial to ensure that your domain points to the correct DNS servers,
    /// which is essential for routing traffic accurately and implementing
    /// changes in DNS management.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:write">domains:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for updating nameservers for a domain is 5 requests per
    ///   domain, within 300 seconds.
    ///
    ///Sends a `PUT` request to `/v1/domains/{domain}/nameservers`
    ///
    ///Arguments:
    /// - `domain`: The domain whose nameservers should be updated. The domain
    ///   name must be provided in a fully qualified domain format.
    /// - `body`: Provides the new set of nameservers for the domain. The
    ///   request must include a list of nameservers that will replace the
    ///   existing ones.
    pub async fn set_domain_nameservers<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::DomainNameServersConfigurationRequest,
    ) -> Result<ResponseValue<types::DomainNameServersConfigurationResponse>, Error<ByteStream>>
    {
        let url = format!(
            "{}/v1/domains/{}/nameservers",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            operation_id: "set_domain_nameservers",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get personal nameservers on a domain
    ///
    ///Get the personal nameservers for a specific domain.
    ///
    ///Personal nameservers are a combination of A records set for a specific
    /// host and glue records set for the domain on the registry.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:read">domains:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for fetching personal nameservers for a domain is 5 requests
    ///   per domain within 300 seconds.
    ///
    ///Sends a `GET` request to `/v1/domains/{domain}/personal-nameservers`
    ///
    ///Arguments:
    /// - `domain`: A domain name whose details should be fetched. The domain
    ///   name must be provided in a fully qualified domain format.
    pub async fn get_domain_personal_nameservers<'a>(
        &'a self,
        domain: &'a str,
    ) -> Result<ResponseValue<types::PersonalNameservers>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/personal-nameservers",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            operation_id: "get_domain_personal_nameservers",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get personal nameservers host configuration
    ///
    ///<div style="font-size: 1.8em; color: #f06700; font-weight:bold;
    /// padding-bottom: 20px; line-height: 1.4em"> This API is under
    /// development and currently returns an HTTP 501 status.
    /// <p style="font-size: 0.7em;">The provided information is for preliminary
    /// familiarization only. Once the API is implemented, this notice will be
    /// removed.</p> </div>
    /// Get the configuration for personal nameservers associated with a
    /// specific domain.
    ///
    ///Personal nameservers consist of A records for a specific host and glue
    /// records at the domain registry.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:read">domains:read</a>
    ///
    ///Sends a `GET` request to
    /// `/v1/domains/{domain}/personal-nameservers/{currentHost}`
    ///
    ///Arguments:
    /// - `domain`: Domain name whose details should be fetched. The domain name
    ///   must be provided in a fully qualified domain format.
    /// - `current_host`: The host name part of the nameserver. For example, for
    ///   `spaceship.dev` domain and `ns1.spaceship.dev` fully qualified name,
    ///   the host is `ns1`.
    pub async fn get_domain_personal_nameserver_host_info<'a>(
        &'a self,
        domain: &'a str,
        current_host: &'a str,
    ) -> Result<ResponseValue<types::HostNameServers>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/personal-nameservers/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
            encode_path(&current_host.to_string()),
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
            operation_id: "get_domain_personal_nameserver_host_info",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update personal nameservers host configuration
    ///
    ///Update the configuration for personal nameservers associated with a
    /// specific domain.
    ///
    ///**If the host in the request body differs from the host in the path,
    /// this operation will rename the host and update the IP addresses, making
    /// the old host return a 404.**
    ///
    ///Personal nameservers are a combination of A records set for a specific
    /// host and glue records set for the domain on the registry.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:write">domains:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for updating the personal nameservers for a domain is 10
    ///   requests per domain, within 300 seconds.
    ///
    ///Sends a `PUT` request to
    /// `/v1/domains/{domain}/personal-nameservers/{currentHost}`
    ///
    ///Arguments:
    /// - `domain`: The domain name in fully qualified format whose nameserver
    ///   configuration should be updated
    /// - `current_host`: The host name part of the nameserver. For example, for
    ///   `spaceship.dev` domain and `ns1.spaceship.dev` fully qualified name,
    ///   the host is `ns1`.
    /// - `body`
    pub async fn update_domain_personal_nameserver_host_info<'a>(
        &'a self,
        domain: &'a str,
        current_host: &'a str,
        body: &'a types::PersonalNameserverRecord,
    ) -> Result<ResponseValue<types::PersonalNameserverRecord>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/personal-nameservers/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
            encode_path(&current_host.to_string()),
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
            operation_id: "update_domain_personal_nameserver_host_info",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete personal nameservers host configuration
    ///
    ///Delete the personal nameservers host configuration for a specific
    /// domain.
    ///
    ///Personal nameservers are a combination of A records set for a specific
    /// host and glue records set for the domain on the registry.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:write">domains:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for deleting the personal nameservers host configuration for
    ///   a domain is 10 requests per domain, within 300 seconds.
    ///
    ///Sends a `DELETE` request to
    /// `/v1/domains/{domain}/personal-nameservers/{currentHost}`
    ///
    ///Arguments:
    /// - `domain`: Domain name in a fully qualified format whose personal
    ///   nameservers host should be deleted
    /// - `current_host`: The host name part of the nameserver. For example, for
    ///   `spaceship.dev` domain and `ns1.spaceship.dev` fully qualified name,
    ///   the host is `ns1`.
    pub async fn delete_domain_personal_nameserver_host_info<'a>(
        &'a self,
        domain: &'a str,
        current_host: &'a str,
    ) -> Result<ResponseValue<()>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/personal-nameservers/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
            encode_path(&current_host.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self.client.delete(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "delete_domain_personal_nameserver_host_info",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            204u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update domain email protection preference
    ///
    ///Allows the modification of a domain's email protection preference.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:write">domains:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for updating the domain email protection preference for a
    ///   domain is 5 requests per domain, within 300 seconds.
    ///
    ///Sends a `PUT` request to
    /// `/v1/domains/{domain}/privacy/email-protection-preference`
    ///
    ///Arguments:
    /// - `domain`: The domain whose email protection preference is being
    ///   updated.
    /// - `body`: Provides the new email protection level for the domain.
    pub async fn update_domain_email_protection_preference<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::DomainsDomainEmailProtectionPreference,
    ) -> Result<ResponseValue<()>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/privacy/email-protection-preference",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "update_domain_email_protection_preference",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            204u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update domain privacy preference
    ///
    ///Allows the modification of a domain's privacy preference.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:write">domains:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for updating the privacy preference for a domain is 5
    ///   requests per domain, within 300 seconds.
    ///
    ///Sends a `PUT` request to `/v1/domains/{domain}/privacy/preference`
    ///
    ///Arguments:
    /// - `domain`: The domain whose privacy preference is being updated.
    /// - `body`: Provides the new privacy level for the domain.
    pub async fn update_domain_privacy_preference<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::DomainsDomainPrivacyPreference,
    ) -> Result<ResponseValue<()>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/privacy/preference",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "update_domain_privacy_preference",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            204u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Requests domain renewal
    ///
    ///Requests domain renewal.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:billing">domains:billing</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit to renew domains is 30 requests per user, within 30 seconds.
    ///
    ///Sends a `POST` request to `/v1/domains/{domain}/renew`
    ///
    ///Arguments:
    /// - `domain`: The domain name to be renewed.
    /// - `body`
    pub async fn domain_renew<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::DomainsDomainRenewalRequestInfo,
    ) -> Result<ResponseValue<()>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/renew",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "domain_renew",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            202u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Requests domain restoration
    ///
    ///Requests domain restoration.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:billing">domains:billing</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit to restore domains is 30 requests per user, within 30
    ///   seconds.
    ///
    ///Sends a `POST` request to `/v1/domains/{domain}/restore`
    ///
    ///Arguments:
    /// - `domain`: The domain name to be restored.
    pub async fn domain_restore<'a>(
        &'a self,
        domain: &'a str,
    ) -> Result<ResponseValue<()>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/restore",
            self.baseurl,
            encode_path(&domain.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self.client.post(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "domain_restore",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            202u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get the details of the domain transfer
    ///
    ///Get the details of the ongoing domain transfer.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:transfer">domains:transfer</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for requesting transfer info is 5 requests per domain,
    ///   within 300 seconds.
    ///
    ///Sends a `GET` request to `/v1/domains/{domain}/transfer`
    ///
    ///Arguments:
    /// - `domain`: The domain whose transfer details are to be returned.
    pub async fn get_transfer_info<'a>(
        &'a self,
        domain: &'a str,
    ) -> Result<ResponseValue<types::DomainsDomainTransferDetailsResponse>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/transfer",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            operation_id: "get_transfer_info",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Requests domain transfer
    ///
    ///Requests domain transfer.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:billing">domains:billing</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit to transfer domains is 30 requests per user, within 30
    ///   seconds.
    ///
    ///Sends a `POST` request to `/v1/domains/{domain}/transfer`
    ///
    ///Arguments:
    /// - `domain`: The domain name for transfer.
    /// - `body`
    pub async fn transfer_request<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::DomainTransferRequest,
    ) -> Result<ResponseValue<()>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/transfer",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "transfer_request",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            202u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get domain auth code
    ///
    ///Get the domain's auth code, also known as EPP code, authorization code,
    /// transfer code or Auth-Info Code.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:transfer">domains:transfer</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for obtaining auth codes is 5 requests per domain, within
    ///   300 seconds.
    ///
    ///Sends a `GET` request to `/v1/domains/{domain}/transfer/auth-code`
    ///
    ///Arguments:
    /// - `domain`: The domain whose auth code is to be returned.
    pub async fn get_auth_code<'a>(
        &'a self,
        domain: &'a str,
    ) -> Result<ResponseValue<types::DomainsDomainAuthCodeResponse>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/transfer/auth-code",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            operation_id: "get_auth_code",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update domain transfer lock
    ///
    ///Allows the modification of a domain transfer lock.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/domains:transfer">domains:transfer</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for updating the transfer lock for a domain is 5 requests
    ///   per domain within 300 seconds.
    ///
    ///Sends a `PUT` request to `/v1/domains/{domain}/transfer/lock`
    ///
    ///Arguments:
    /// - `domain`: The domain whose transfer lock is being updated.
    /// - `body`: Provides the new trnasfer lock for the domain.
    pub async fn update_transfer_lock<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::DomainsDomainTransferLock,
    ) -> Result<ResponseValue<types::DomainsDomainTransferLock>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/domains/{}/transfer/lock",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            operation_id: "update_transfer_lock",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Create a checkout link
    ///
    ///Creates a new checkout link for a domain listed in SellerHub.
    ///
    ///Checkout links provide a direct URL that can be shared with potential
    /// buyers, allowing them to purchase a domain at a specified price without
    /// requiring them to browse the marketplace.
    ///
    ///### Use Cases
    /// - Share a personalized purchase link with an interested buyer
    /// - Create campaign-specific links for marketing purposes
    /// - Generate time-limited offers for domain sales
    ///
    ///### Notes
    /// - The domain must already be listed in SellerHub before creating a
    ///   checkout link
    /// - The base price in the checkout link can differ from the listing price
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/sellerhub:write">sellerhub:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for creating a checkout link is 300 requests per user,
    ///   within 300 seconds.
    ///
    ///Sends a `POST` request to `/v1/sellerhub/checkout-links`
    ///
    ///Arguments:
    /// - `body`: Create checkout link request payload
    pub async fn create_checkout_link<'a>(
        &'a self,
        body: &'a types::CreateCheckoutLinkRequest,
    ) -> Result<ResponseValue<types::CreateCheckoutLinkResponse>, Error<ByteStream>> {
        let url = format!("{}/v1/sellerhub/checkout-links", self.baseurl,);
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
            operation_id: "create_checkout_link",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get SellerHub domains list
    ///
    ///Returns a list of domains in SellerHub.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/sellerhub:read">sellerhub:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for fetching a SellerHub domain list is 300 requests per
    ///   user, within 300 seconds.
    ///
    ///Sends a `GET` request to `/v1/sellerhub/domains`
    ///
    ///Arguments:
    /// - `skip`: Number of response items to skip
    /// - `take`: Number of response items per page
    pub async fn get_seller_hub_domain_list<'a>(
        &'a self,
        skip: i32,
        take: ::std::num::NonZeroU32,
    ) -> Result<ResponseValue<types::GetSellerHubDomainListResponse>, Error<ByteStream>> {
        let url = format!("{}/v1/sellerhub/domains", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("skip", &skip))
            .query(&progenitor_client::QueryParam::new("take", &take))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_seller_hub_domain_list",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Create a SellerHub domain
    ///
    ///Creates a new domain listing in SellerHub.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/sellerhub:write">sellerhub:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for creating a SellerHub domain is 300 requests per user,
    ///   within 300 seconds.
    ///
    ///Sends a `POST` request to `/v1/sellerhub/domains`
    ///
    ///Arguments:
    /// - `body`: Create request payload
    pub async fn create_seller_hub_domain<'a>(
        &'a self,
        body: &'a types::CreateSellerHubDomainRequest,
    ) -> Result<ResponseValue<types::SellerHubDomainResponse>, Error<ByteStream>> {
        let url = format!("{}/v1/sellerhub/domains", self.baseurl,);
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
            operation_id: "create_seller_hub_domain",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            201u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get a specific SellerHub domain
    ///
    ///Retrieves detailed information about a specific domain in SellerHub.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/sellerhub:read">sellerhub:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for obtaining info for a SellerHub domain is 300 requests
    ///   per domain, within 300 seconds.
    ///
    ///Sends a `GET` request to `/v1/sellerhub/domains/{domain}`
    ///
    ///Arguments:
    /// - `domain`: Domain name
    pub async fn get_seller_hub_domain<'a>(
        &'a self,
        domain: &'a str,
    ) -> Result<ResponseValue<types::SellerHubDomainResponse>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/sellerhub/domains/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            operation_id: "get_seller_hub_domain",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Delete a SellerHub domain
    ///
    ///Removes a domain from SellerHub.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/sellerhub:write">sellerhub:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for deleting a SellerHub domain is 300 requests per domain,
    ///   within 300 seconds.
    ///
    ///Sends a `DELETE` request to `/v1/sellerhub/domains/{domain}`
    ///
    ///Arguments:
    /// - `domain`: Domain name
    pub async fn delete_seller_hub_domain<'a>(
        &'a self,
        domain: &'a str,
    ) -> Result<ResponseValue<()>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/sellerhub/domains/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self.client.delete(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "delete_seller_hub_domain",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            204u16 => Ok(ResponseValue::empty(response)),
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Update a SellerHub domain
    ///
    ///Updates the configuration and pricing settings for a specific domain in
    /// SellerHub. Only provided fields will be updated.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/sellerhub:write">sellerhub:write</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for updating a SellerHub domain is 300 requests per domain,
    ///   within 300 seconds.
    ///
    ///Sends a `PATCH` request to `/v1/sellerhub/domains/{domain}`
    ///
    ///Arguments:
    /// - `domain`: Update request payload
    /// - `body`
    pub async fn update_seller_hub_domain<'a>(
        &'a self,
        domain: &'a str,
        body: &'a types::UpdateSellerHubDomainRequest,
    ) -> Result<ResponseValue<types::SellerHubDomainResponse>, Error<ByteStream>> {
        let url = format!(
            "{}/v1/sellerhub/domains/{}",
            self.baseurl,
            encode_path(&domain.to_string()),
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
            operation_id: "update_seller_hub_domain",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }

    ///Get verification records
    ///
    ///Returns verification options for domain ownership verification in
    /// SellerHub.
    ///
    ///The response contains one or more verification options. You can choose
    /// ANY of the returned options (OR logic) that works best for your DNS
    /// setup. Within a chosen option, ALL records must be created (AND
    /// logic).
    ///
    ///**Important:** The returned verification records are associated with
    /// your SellerHub account and can be used to verify ownership of ALL
    /// domains you want to add to SellerHub. You only need to create these
    /// DNS records once per account, not separately for each domain.
    ///
    ///**Usage:**
    /// 1. Call this endpoint to retrieve verification options
    /// 2. Select ONE option from `options` array
    /// 3. Create ALL DNS records listed in your selected option's `records`
    ///    array on any domain
    /// 4. Wait for DNS propagation; the system will verify your domain
    /// 5. The same records can be used for verifying additional domains in your
    ///    SellerHub account
    ///
    ///This verification process is required for adding external domains to
    /// SellerHub.
    ///
    ///### Required Permissions
    ///
    /// - <a href="#scopes/sellerhub:read">sellerhub:read</a>
    ///
    ///### Rate Limits
    ///
    /// * The limit for fetching verification records is 10 requests per user,
    ///   within 60 seconds.
    ///
    ///Sends a `GET` request to `/v1/sellerhub/verification-records`
    pub async fn get_verification_records<'a>(
        &'a self,
    ) -> Result<ResponseValue<types::SellerHubVerificationResponse>, Error<ByteStream>> {
        let url = format!("{}/v1/sellerhub/verification-records", self.baseurl,);
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
            operation_id: "get_verification_records",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            401u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            403u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            404u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            422u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            429u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            500u16 => Err(Error::ErrorResponse(ResponseValue::stream(response))),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
}

/// Items consumers will typically use such as the Client.
pub mod prelude {
    #[allow(unused_imports)]
    pub use super::Client;
}
