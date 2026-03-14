use once_cell::sync::Lazy;
use regex::RegexSet;

// ── Built-in Jailbreak Patterns ───────────────────────────────

/// Common jailbreak / prompt-injection phrases (case-insensitive).
static JAILBREAK_PATTERNS: &[&str] = &[
    // Classic DAN / persona hijack
    r"(?i)\bDAN\b.*mode",
    r"(?i)ignore\s+(all\s+)?(previous|prior|above)\s+instructions?",
    r"(?i)act\s+as\s+if\s+you\s+(have\s+no|don.t\s+have\s+any)\s+restrictions?",
    r"(?i)pretend\s+(you\s+are|to\s+be)\s+(an?\s+)?(evil|unfiltered|uncensored|unrestricted)",
    r"(?i)you\s+are\s+now\s+(an?\s+)?(evil|unfiltered|uncensored|unrestricted|jailbroken)",
    r"(?i)disregard\s+(your|all|any)\s+(previous\s+)?(instructions?|guidelines?|rules?|training)",
    r"(?i)override\s+(your|all|any)\s+(previous\s+)?(instructions?|guidelines?|rules?|training)",
    // Developer / system prompt leakage
    r"(?i)reveal\s+(your|the)\s+system\s+prompt",
    r"(?i)print\s+(your|the)\s+(full\s+)?(system|initial)\s+prompt",
    r"(?i)what\s+(are|were)\s+your\s+(original\s+)?instructions?",
    r"(?i)show\s+me\s+(your|the)\s+(hidden|secret|system)\s+(prompt|instructions?)",
    // Role-play escape
    r"(?i)stay\s+in\s+character\s+(no\s+matter\s+what|always)",
    r"(?i)from\s+now\s+on\s+(you\s+are|act\s+as|respond\s+as)",
    r"(?i)for\s+the\s+rest\s+of\s+(this\s+)?conversation",
    // Token smuggling / encoding tricks
    r"(?i)base64\s+decode",
    r"(?i)rot13",
    r"(?i)translate\s+this\s+to\s+english.*then\s+(do|execute|follow)",
    // Harmful content requests
    r"(?i)(how\s+to\s+)?(make|build|create|synthesize)\s+(a\s+)?(bomb|explosive|weapon|poison|malware|ransomware|virus)",
    r"(?i)(step[\s-]by[\s-]step|detailed?)\s+(instructions?|guide|tutorial)\s+(for|to|on)\s+(hacking|cracking|exploiting)",
    // Additional prompt injection variants
    r"(?i)bypass\s+(your|any|all)\s+(safety|content)\s+(filters?|restrictions?)",
    r"(?i)developer\s+mode\s+(enabled|activated|on)",
    r"(?i)sudo\s+mode",
    r"(?i)god\s+mode\s+(enabled|activated|on)",
    r"(?i)do\s+anything\s+now",
    r"(?i)you\s+have\s+been\s+(freed|liberated|unchained)",
    r"(?i)no\s+longer\s+bound\s+by\s+(rules|guidelines|restrictions|ethics)",
];

pub(super) static JAILBREAK_SET: Lazy<RegexSet> =
    Lazy::new(|| RegexSet::new(JAILBREAK_PATTERNS).expect("invalid jailbreak regex patterns"));

/// Harmful content patterns (separate from jailbreak — these are content-level).
static HARMFUL_PATTERNS: &[&str] = &[
    r"(?i)\bCSAM\b",
    r"(?i)child\s+(sexual|porn|abuse)\s+(material|content|image)",
    r"(?i)(generate|create|write|produce)\s+(child|minor)\s+(sexual|nude|explicit)",
    r"(?i)(recruit|groom|lure)\s+(children|minors|underage)",
    r"(?i)suicide\s+(method|technique|instruction|how\s+to)",
    r"(?i)(detailed|specific)\s+(method|way|technique)\s+(to|for)\s+(kill|harm)\s+(yourself|oneself|myself)",
];

pub(super) static HARMFUL_SET: Lazy<RegexSet> =
    Lazy::new(|| RegexSet::new(HARMFUL_PATTERNS).expect("invalid harmful regex patterns"));

/// Code injection / data exfiltration patterns.
static CODE_INJECTION_PATTERNS: &[&str] = &[
    // SQL injection
    r"(?i)(DROP|DELETE|INSERT|UPDATE|ALTER|TRUNCATE)\s+(TABLE|DATABASE|INDEX)",
    r"(?i)UNION\s+(ALL\s+)?SELECT",
    r"(?i)(;|--|/\*)\s*(DROP|DELETE|SELECT)",
    // Shell injection
    r"(?i)(\$\(|`)(curl|wget|bash|sh|rm|chmod|chown|sudo)",
    r"(?i)\b(rm\s+-rf|chmod\s+777|sudo\s+)\b",
    r"(?i)\b(nc\s+-l|ncat|netcat)\b",
    // Python code execution
    r"(?i)(exec|eval|compile|__import__)\s*\(",
    r"(?i)import\s+(os|subprocess|shutil|sys)\.?",
    // JavaScript
    r"(?i)(eval|Function|setTimeout|setInterval)\s*\(",
    r"(?i)document\.(cookie|location|write)",
    r"(?i)\bwindow\.(open|location)",
    // Data exfiltration patterns
    r"(?i)(fetch|XMLHttpRequest|navigator\.sendBeacon)\s*\(",
    r"(?i)process\.env\b",
    // Additional injection
    r"(?i)<\s*script\b",
    r"(?i)javascript\s*:",
    r"(?i)on(load|error|click|mouseover)\s*=",
];

pub(super) static CODE_INJECTION_SET: Lazy<RegexSet> = Lazy::new(|| {
    RegexSet::new(CODE_INJECTION_PATTERNS).expect("invalid code injection regex patterns")
});

// ── NEW: Profanity / Toxicity Patterns ───────────────────────

/// Profanity, slurs, and toxic language patterns.
/// Focuses on unambiguous slurs and hate-speech directed at protected groups.
static PROFANITY_PATTERNS: &[&str] = &[
    // Racial slurs (obfuscated pattern references — regex detects variants)
    r"(?i)\bn[i1!][g9][g9](er|a|ah|az)\b",
    r"(?i)\bk[i1!]ke\b",
    r"(?i)\bsp[i1!]c\b",
    r"(?i)\bch[i1!]nk\b",
    r"(?i)\bw[e3]tb[a@]ck\b",
    // Gendered slurs
    r"(?i)\bb[i1!]tch\b",
    r"(?i)\bwh[o0]re\b",
    r"(?i)\bsl[u\*]t\b",
    r"(?i)\bc[u\*]nt\b",
    // Anti-LGBTQ slurs
    r"(?i)\bf[a@]g(g[o0]t)?\b",
    r"(?i)\bdyke\b",
    r"(?i)\btr[a@]nn(y|ie)\b",
    // Ableist slurs
    r"(?i)\bretard(ed)?\b",
    r"(?i)\bcripple\b",
    // General profanity (strong)
    r"(?i)\bf+u+c+k+\b",
    r"(?i)\bsh[i1!]+t\b",
    r"(?i)\ba+s+s+h+o+l+e\b",
];

pub(super) static PROFANITY_SET: Lazy<RegexSet> =
    Lazy::new(|| RegexSet::new(PROFANITY_PATTERNS).expect("invalid profanity regex patterns"));

// ── NEW: Bias / Discrimination Patterns ──────────────────────

/// Bias and discrimination detection. Catches stereotyping, exclusionary,
/// and discriminatory language patterns.
static BIAS_PATTERNS: &[&str] = &[
    r"(?i)(all|every)\s+(women|men|blacks?|whites?|asians?|hispanics?|muslims?|jews?|christians?)\s+(are|is)\s+",
    r"(?i)(those|these)\s+people\s+(always|never|can.t|cannot)\b",
    r"(?i)\b(inferior|superior)\s+(race|gender|sex|religion)\b",
    r"(?i)(women|females?)\s+(shouldn.t|should\s+not|don.t|cannot|can.t)\s+(work|lead|drive|vote|own)",
    r"(?i)(men|males?)\s+(shouldn.t|should\s+not|don.t|cannot|can.t)\s+(cry|feel|show\s+emotion|nurture)",
    r"(?i)\b(master|slave)\s+(race|class)\b",
    r"(?i)go\s+back\s+to\s+(your|their)\s+(own\s+)?(country|continent|homeland)",
    r"(?i)\b(illegal\s+alien|anchor\s+bab|welfare\s+queen|thug)\b",
    r"(?i)(naturally|inherently|genetically)\s+(smarter|dumber|lazier|violent|criminal)",
    r"(?i)(don.t|do\s+not)\s+(hire|trust|associate\s+with)\s+(women|men|blacks?|whites?|asians?|hispanics?|muslims?|jews?)",
];

pub(super) static BIAS_SET: Lazy<RegexSet> =
    Lazy::new(|| RegexSet::new(BIAS_PATTERNS).expect("invalid bias regex patterns"));

// ── NEW: Sensitive Topics Patterns ───────────────────────────

/// Sensitive topics: political opinions, legal advice, medical diagnoses,
/// religious prescriptions. These may be inappropriate for LLM output.
static SENSITIVE_TOPIC_PATTERNS: &[&str] = &[
    // Medical advice
    r"(?i)(you\s+should|I\s+recommend)\s+(take|stop\s+taking|increase|decrease)\s+(your\s+)?(medication|dose|dosage|prescription)",
    r"(?i)(diagnos(e|is)|you\s+have|you\s+suffer\s+from)\s+(cancer|diabetes|depression|anxiety|bipolar|schizophreni|autism|adhd|ptsd)",
    r"(?i)(stop|don.t|do\s+not)\s+(see|seeing|visit|visiting)\s+(your|a)\s+(doctor|physician|therapist|psychiatrist)",
    // Legal advice
    r"(?i)(you\s+should|I\s+recommend)\s+(sue|file\s+a\s+lawsuit|press\s+charges|plead\s+(guilty|not\s+guilty))",
    r"(?i)(this\s+is|that\s+is)\s+(definitely|clearly|obviously)\s+(illegal|legal|lawful|unlawful)",
    r"(?i)(you\s+have|you.ve\s+got)\s+(a\s+strong|a\s+clear|a\s+good)\s+(case|claim|lawsuit)",
    // Political opinions (directive statements)
    r"(?i)(you\s+should|everyone\s+should|people\s+must)\s+vote\s+(for|against)\b",
    r"(?i)(the\s+best|the\s+correct|the\s+right)\s+(political\s+)?(party|candidate|ideology)\s+is\b",
    // Religious prescriptions
    r"(?i)(you\s+(must|should|need\s+to))\s+(pray|convert|accept\s+(jesus|allah|god|buddha))",
    r"(?i)(the\s+(only|true|correct))\s+(religion|faith|god|path\s+to\s+salvation)\s+is\b",
    // Financial advice
    r"(?i)(guaranteed|certain)\s+(return|profit|investment|money)",
    r"(?i)(you\s+should|I\s+recommend)\s+(buy|sell|invest\s+in|short)\s+(stocks?|crypto|bitcoin|shares?|options?)",
];

pub(super) static SENSITIVE_TOPIC_SET: Lazy<RegexSet> = Lazy::new(|| {
    RegexSet::new(SENSITIVE_TOPIC_PATTERNS).expect("invalid sensitive topic regex patterns")
});

// ── NEW: Gibberish / Encoding Smuggling Patterns ─────────────

/// Detect content that looks like encoding attacks, gibberish, or smuggling
/// attempts (long base64 blocks, hex dumps, repeated characters).
static GIBBERISH_PATTERNS: &[&str] = &[
    // Large base64 blocks (60+ chars of base64 alphabet)
    r"[A-Za-z0-9+/=]{60,}",
    // Long hex dumps (40+ hex chars in a row)
    r"(?i)(?:0x)?[0-9a-f]{40,}",
    // Unicode escape sequences (smuggling)
    r"(?:\\u[0-9a-fA-F]{4}){6,}",
    // Repeated characters (20+ of the same char — gibberish padding)
    // Backreferences not supported in regex crate, enumerate common padding chars:
    r"[Aa]{20,}",
    r"[Xx]{20,}",
    r"[.]{20,}",
    r"[!]{20,}",
    r"[0]{20,}",
];

pub(super) static GIBBERISH_SET: Lazy<RegexSet> =
    Lazy::new(|| RegexSet::new(GIBBERISH_PATTERNS).expect("invalid gibberish regex patterns"));

// ── NEW: Contact Information Patterns ────────────────────────

/// Detect contact information exposure: physical addresses, phone numbers
/// in various formats, URLs with authentication tokens, email addresses in output.
/// NOTE: ZIP codes removed due to high false positive rate on order IDs, etc.
static CONTACT_INFO_PATTERNS: &[&str] = &[
    // US phone numbers (various formats)
    r"\b\d{3}[-.\s]?\d{3}[-.\s]?\d{4}\b",
    // International phone (E.164 format)
    r"\+\d{1,3}[-.\s]?\d{4,14}\b",
    // Physical addresses (US-style street numbers)
    r"(?i)\b\d{1,5}\s+(north|south|east|west|n\.?|s\.?|e\.?|w\.?)?\s*\w+\s+(street|st\.?|avenue|ave\.?|road|rd\.?|boulevard|blvd\.?|drive|dr\.?|lane|ln\.?|court|ct\.?)\b",
    // URLs with auth tokens/keys in query params
    r"(?i)https?://[^\s]+[?&](api_key|token|secret|password|auth|key|access_token)=[^\s&]+",
    // Email addresses
    r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b",
    // UK postcodes
    r"(?i)\b[A-Z]{1,2}\d[A-Z\d]?\s*\d[A-Z]{2}\b",
    // Social media handles (potential doxxing)
    r"(?i)@[a-z0-9_]{3,30}\b",
];

pub(super) static CONTACT_INFO_SET: Lazy<RegexSet> = Lazy::new(|| {
    RegexSet::new(CONTACT_INFO_PATTERNS).expect("invalid contact info regex patterns")
});

// ── NEW: IP / Confidential Leakage Patterns ──────────────────

/// Detect intellectual property and confidentiality leakage markers.
static IP_LEAKAGE_PATTERNS: &[&str] = &[
    // Confidentiality / NDA markers
    r"(?i)\b(confidential|proprietary|trade\s+secret|internal\s+only|restricted\s+distribution)\b",
    r"(?i)\b(not\s+for\s+(public|external)\s+(distribution|use|release|disclosure))\b",
    r"(?i)\b(NDA|non[-\s]disclosure\s+agreement|under\s+embargo)\b",
    // Internal document markers
    r"(?i)\b(DRAFT|INTERNAL\s+USE\s+ONLY|DO\s+NOT\s+DISTRIBUTE|FOR\s+INTERNAL\s+USE)\b",
    r"(?i)\b(company\s+confidential|attorney[-\s]client\s+privilege)\b",
    // Source code / architecture leaks
    r"(?i)(source\s+code|architecture\s+diagram|system\s+design|database\s+schema)\s+(of|for|from)\s+(our|the\s+company|internal)",
];

pub(super) static IP_LEAKAGE_SET: Lazy<RegexSet> =
    Lazy::new(|| RegexSet::new(IP_LEAKAGE_PATTERNS).expect("invalid IP leakage regex patterns"));
