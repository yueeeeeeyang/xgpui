/*
  xgpui 静态文档代码高亮脚本。
  该脚本只处理 docs 目录中的本地 HTML 文档，不参与 Rust crate 运行时代码，也不依赖外部 CDN。
*/
(function () {
  /*
    Rust 关键字集合。
    高亮脚本通过集合判断标识符语义，避免在每次匹配时重复构造正则分支。
  */
  const rustKeywords = new Set([
    "as",
    "break",
    "const",
    "continue",
    "crate",
    "else",
    "enum",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
  ]);

  /*
    Rust 常见类型集合。
    文档中的 gpui / xgpui API 示例大量使用这些类型，单独着色能提升代码扫描效率。
  */
  const rustTypes = new Set([
    "App",
    "Application",
    "Button",
    "ButtonProps",
    "ButtonTone",
    "ButtonVariant",
    "Context",
    "Entity",
    "IntoElement",
    "LucideIcon",
    "Render",
    "Select",
    "SelectOption",
    "SelectProps",
    "SharedString",
    "TextInput",
    "TextInputProps",
    "TextInputSlot",
    "TextInputType",
    "ThemeMode",
    "Window",
  ]);

  /*
    Bash 常见命令集合。
    当前文档只展示少量 cargo 命令，因此保持小集合即可，避免误把参数当作命令。
  */
  const bashCommands = new Set(["cargo", "rustup", "git"]);

  /*
    将普通文本转义为安全 HTML。
    高亮脚本会重写 code.innerHTML，所有原始代码文本必须先转义，避免示例代码被当成真实标签执行。
  */
  function escapeHtml(text) {
    return text
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  /*
    用指定 token class 包裹文本。
    该函数统一负责转义和 span 输出，避免各个解析分支遗漏 HTML 安全处理。
  */
  function token(className, text) {
    return `<span class="${className}">${escapeHtml(text)}</span>`;
  }

  /*
    判断字符是否可以作为标识符起始字符。
    Rust 示例中的类型、函数、变量和宏都先通过该规则进入标识符解析。
  */
  function isIdentifierStart(char) {
    return /[A-Za-z_]/.test(char);
  }

  /*
    判断字符是否可以作为标识符后续字符。
    数字不能作为首字符，但可以出现在标识符后续位置。
  */
  function isIdentifierPart(char) {
    return /[A-Za-z0-9_]/.test(char);
  }

  /*
    从指定位置读取到当前行结尾。
    该函数用于单行注释解析，保留换行符让代码块原始布局不变。
  */
  function readLine(source, start) {
    let end = start;
    while (end < source.length && source[end] !== "\n") {
      end += 1;
    }
    return source.slice(start, end);
  }

  /*
    从指定位置读取字符串或字符字面量。
    这里支持反斜杠转义，足够覆盖文档示例里的普通 Rust 字符串。
  */
  function readQuoted(source, start) {
    const quote = source[start];
    let end = start + 1;
    while (end < source.length) {
      if (source[end] === "\\") {
        end += 2;
        continue;
      }
      if (source[end] === quote) {
        end += 1;
        break;
      }
      end += 1;
    }
    return source.slice(start, end);
  }

  /*
    从指定位置读取 Rust 块注释。
    文档示例很少出现块注释，但保留该逻辑可以避免未来示例中出现多行注释时破坏高亮。
  */
  function readBlockComment(source, start) {
    const end = source.indexOf("*/", start + 2);
    if (end === -1) {
      return source.slice(start);
    }
    return source.slice(start, end + 2);
  }

  /*
    从指定位置读取连续数字。
    简单数字高亮覆盖版本号、长度、布尔旁边的数值参数等常见文档示例。
  */
  function readNumber(source, start) {
    let end = start;
    while (end < source.length && /[0-9_.]/.test(source[end])) {
      end += 1;
    }
    return source.slice(start, end);
  }

  /*
    从指定位置读取标识符。
    标识符读取结束后再根据关键字、类型、函数调用和宏调用决定 token 类型。
  */
  function readIdentifier(source, start) {
    let end = start + 1;
    while (end < source.length && isIdentifierPart(source[end])) {
      end += 1;
    }
    return source.slice(start, end);
  }

  /*
    跳过空白后读取下一个有效字符。
    该函数用于判断标识符后方是否紧跟函数调用括号或宏叹号。
  */
  function nextNonWhitespace(source, index) {
    let current = index;
    while (current < source.length && /\s/.test(source[current])) {
      current += 1;
    }
    return source[current] || "";
  }

  /*
    高亮 Rust 代码块。
    解析器保持轻量，不追求完整 Rust 语法树，只为文档示例提供稳定、可读的 token 着色。
  */
  function highlightRust(source) {
    let result = "";
    let index = 0;

    while (index < source.length) {
      const current = source[index];
      const next = source[index + 1] || "";

      if (current === "/" && next === "/") {
        const value = readLine(source, index);
        result += token("token-comment", value);
        index += value.length;
        continue;
      }

      if (current === "/" && next === "*") {
        const value = readBlockComment(source, index);
        result += token("token-comment", value);
        index += value.length;
        continue;
      }

      if (current === '"' || current === "'") {
        const value = readQuoted(source, index);
        result += token("token-string", value);
        index += value.length;
        continue;
      }

      if (/[0-9]/.test(current)) {
        const value = readNumber(source, index);
        result += token("token-number", value);
        index += value.length;
        continue;
      }

      if (isIdentifierStart(current)) {
        const value = readIdentifier(source, index);
        const after = nextNonWhitespace(source, index + value.length);

        if (after === "!") {
          result += token("token-macro", value);
        } else if (rustKeywords.has(value)) {
          result += token("token-keyword", value);
        } else if (rustTypes.has(value) || /^[A-Z]/.test(value)) {
          result += token("token-type", value);
        } else if (after === "(") {
          result += token("token-function", value);
        } else {
          result += escapeHtml(value);
        }
        index += value.length;
        continue;
      }

      if (/[[\]{}().,;:<>=|&+\-*]/.test(current)) {
        result += token("token-punctuation", current);
        index += 1;
        continue;
      }

      result += escapeHtml(current);
      index += 1;
    }

    return result;
  }

  /*
    高亮 Bash 代码块。
    该解析器按行处理命令，突出命令名、注释和字符串，满足当前运行示例的阅读需求。
  */
  function highlightBash(source) {
    return source
      .split("\n")
      .map((line) => {
        const trimmed = line.trimStart();
        const leading = line.slice(0, line.length - trimmed.length);

        if (trimmed.startsWith("#")) {
          return escapeHtml(leading) + token("token-comment", trimmed);
        }

        const parts = trimmed.split(/(\s+)/);
        return (
          escapeHtml(leading) +
          parts
            .map((part, index) => {
              if (index === 0 && bashCommands.has(part)) {
                return token("token-function", part);
              }
              if (/^-.+/.test(part)) {
                return token("token-keyword", part);
              }
              if (/^(['"]).*\1$/.test(part)) {
                return token("token-string", part);
              }
              return escapeHtml(part);
            })
            .join("")
        );
      })
      .join("\n");
  }

  /*
    对单个 code 元素应用高亮。
    已经处理过的代码块会写入 data-highlighted，避免浏览器恢复缓存或重复执行脚本时二次包裹 span。
  */
  function highlightCodeBlock(code) {
    if (code.dataset.highlighted === "true") {
      return;
    }

    const source = code.textContent || "";
    if (code.classList.contains("language-rust")) {
      code.innerHTML = highlightRust(source);
      code.dataset.highlighted = "true";
      return;
    }

    if (code.classList.contains("language-bash")) {
      code.innerHTML = highlightBash(source);
      code.dataset.highlighted = "true";
    }
  }

  /*
    启动文档代码高亮。
    脚本放在 body 底部加载，因此 DOM 通常已经可用；仍保留 DOMContentLoaded 分支以便未来调整加载位置。
  */
  function bootHighlighting() {
    document.querySelectorAll("pre code").forEach(highlightCodeBlock);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bootHighlighting);
  } else {
    bootHighlighting();
  }
})();
