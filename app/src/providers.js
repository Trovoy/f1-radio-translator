// Field keys match the Rust side: key / id / url / model / region / email.
// Secret fields (key) are encrypted with Windows DPAPI when saved.
const KEY = { k: "key", l: "API Key", secret: true };
const URL = (def) => ({ k: "url", l: "Base URL", def });
const MODEL = (def) => ({ k: "model", l: "模型", def });

export const CATEGORIES = { mt: "机器翻译", llm: "大模型", local: "本地与自建" };

export const PROVIDERS = [
  { id: "deepl", n: "DeepL", c: "mt", d: "Free 与 Pro 按 Key 自动识别", f: [KEY] },
  { id: "google", n: "Google 翻译", c: "mt", d: "Cloud Translation API", f: [KEY] },
  { id: "azure", n: "Microsoft 翻译", c: "mt", d: "Azure AI Translator", f: [{ ...KEY, l: "订阅密钥" }, { k: "region", l: "区域", def: "eastasia" }] },
  { id: "baidu", n: "百度翻译", c: "mt", d: "通用文本翻译", f: [{ k: "id", l: "APP ID" }, { ...KEY, l: "密钥" }] },
  { id: "youdao", n: "有道智云", c: "mt", d: "文本翻译", f: [{ k: "id", l: "应用 ID" }, { ...KEY, l: "应用密钥" }] },
  { id: "tencent", n: "腾讯翻译君", c: "mt", d: "机器翻译 TMT", f: [{ k: "id", l: "SecretId" }, { ...KEY, l: "SecretKey" }, { k: "region", l: "地域", def: "ap-guangzhou" }] },
  { id: "aliyun", n: "阿里云翻译", c: "mt", d: "机器翻译通用版", f: [{ k: "id", l: "AccessKey ID" }, { ...KEY, l: "AccessKey Secret" }] },
  { id: "volc", n: "火山翻译", c: "mt", d: "火山引擎机器翻译", f: [{ k: "id", l: "Access Key" }, { ...KEY, l: "Secret Key" }] },
  { id: "mymemory", n: "MyMemory", c: "mt", d: "免密钥，额度有限", f: [{ k: "email", l: "邮箱（可选，提高额度）" }] },
  { id: "openai", n: "OpenAI", c: "llm", d: "GPT 系列", f: [URL("https://api.openai.com/v1"), KEY, MODEL("gpt-6-luna")] },
  { id: "claude", n: "Claude", c: "llm", d: "Anthropic", f: [URL("https://api.anthropic.com"), KEY, MODEL("claude-haiku-5-5")] },
  { id: "gemini", n: "Gemini", c: "llm", d: "Google", f: [KEY, MODEL("gemini-2.5-flash")] },
  { id: "deepseek", n: "DeepSeek", c: "llm", d: "中文自然，价格低", f: [KEY, MODEL("deepseek-chat")] },
  { id: "qwen", n: "通义千问", c: "llm", d: "阿里云百炼", f: [KEY, MODEL("qwen-plus")] },
  { id: "glm", n: "智谱 GLM", c: "llm", d: "glm-4-flash 免费", f: [KEY, MODEL("glm-4-flash")] },
  { id: "kimi", n: "Kimi", c: "llm", d: "月之暗面", f: [KEY, MODEL("moonshot-v1-8k")] },
  { id: "doubao", n: "豆包", c: "llm", d: "火山方舟，模型处填接入点 ID", f: [KEY, { k: "model", l: "接入点 ID" }] },
  { id: "openrouter", n: "OpenRouter", c: "llm", d: "一个 Key 用多家模型", f: [KEY, MODEL("")] },
  { id: "silicon", n: "硅基流动", c: "llm", d: "开源模型托管", f: [KEY, MODEL("Qwen/Qwen2.5-7B-Instruct")] },
  { id: "custom", n: "自定义接口", c: "llm", d: "任意 OpenAI 兼容地址", f: [URL(""), KEY, MODEL("")] },
  { id: "ollama", n: "Ollama", c: "local", d: "本机运行，无需密钥", f: [URL("http://localhost:11434/v1"), MODEL("qwen2.5:7b")] },
  { id: "lmstudio", n: "LM Studio", c: "local", d: "本机运行", f: [URL("http://localhost:1234/v1"), MODEL("")] },
  { id: "libre", n: "LibreTranslate", c: "local", d: "开源，可自建", f: [{ k: "url", l: "服务地址", def: "http://localhost:5000" }, { ...KEY, l: "API Key（可选）" }] },
  { id: "deeplx", n: "DeepLX", c: "local", d: "自建 DeepL 代理", f: [{ k: "url", l: "服务地址", def: "http://localhost:1188/translate" }, { ...KEY, l: "Token（可选）" }] },
];

export const PROVIDER = Object.fromEntries(PROVIDERS.map((p) => [p.id, p]));
export const providerName = (id) => PROVIDER[id]?.n ?? id;

/** Values for a provider, falling back to field defaults. */
export function fieldsFor(settings, id) {
  const saved = settings.providers?.[id] ?? {};
  const out = {};
  for (const f of PROVIDER[id]?.f ?? []) out[f.k] = saved[f.k] ?? f.def ?? "";
  return out;
}

/** A provider counts as configured when every non-optional field has a value. */
export function isConfigured(settings, id) {
  const p = PROVIDER[id];
  if (!p) return false;
  const v = fieldsFor(settings, id);
  return p.f.every((f) => /可选/.test(f.l) || f.k === "email" || (v[f.k] ?? "").trim() !== "");
}
