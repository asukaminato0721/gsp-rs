(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  function buildExpressionRichMarkup(exprLabel: string, valueText: string) {
    if (typeof exprLabel !== "string") {
      return null;
    }
    const richTextNode = ( text) => text
      ? `<Tx${text.split("&").join("＆").split("<").join("＜").split(">").join("＞").split("*").join("\u00b7")}>`
      : "";
    const matchingCloseParen = ( text,  openIndex) => {
      let depth = 0;
      for (let index = openIndex; index < text.length; index += 1) {
        if (text[index] === "(") {
          depth += 1;
        } else if (text[index] === ")") {
          depth -= 1;
          if (depth === 0) return index;
          if (depth < 0) return -1;
        }
      }
      return -1;
    };
    const stripWrappingParens = ( text) => {
      const trimmed = text.trim();
      if (!trimmed.startsWith("(") || !trimmed.endsWith(")")) return trimmed;
      return matchingCloseParen(trimmed, 0) === trimmed.length - 1
        ? trimmed.slice(1, -1)
        : trimmed;
    };
    const renderExpressionPart = ( text) => {
      let output = "";
      let rest = text;
      while (true) {
        const index = rest.indexOf("√(");
        if (index < 0) {
          output += richTextNode(rest);
          return output;
        }
        output += richTextNode(rest.slice(0, index));
        const openIndex = index + 1;
        const closeIndex = matchingCloseParen(rest, openIndex);
        if (closeIndex < 0) {
          output += richTextNode(rest.slice(index));
          return output;
        }
        output += `<R${renderExpressionPart(stripWrappingParens(rest.slice(openIndex + 1, closeIndex)))}>`;
        rest = rest.slice(closeIndex + 1);
      }
    };
    const additiveFraction = exprLabel.match(/^(.*)\s\+\s(.*)\s\/\s(.*)$/);
    if (additiveFraction) {
      const [, prefix, numerator, denominator] = additiveFraction;
      return `<H${renderExpressionPart(`${prefix} + `)}</<H${renderExpressionPart(numerator)}><H${renderExpressionPart(denominator)}>><Tx = ${valueText}>>`;
    }
    const parts = exprLabel.split(" / ");
    if (parts.length === 2) {
      return `<H</<H${renderExpressionPart(stripWrappingParens(parts[0]))}><H${renderExpressionPart(parts[1])}>><Tx = ${valueText}>>`;
    }
    return `<H${renderExpressionPart(exprLabel)}<Tx = ${valueText}>>`;
  }


  function buildRatioValueRichMarkup(name: string, valueText: string) {
    if (typeof name !== "string") {
      return null;
    }
    const trimmed = name.trim();
    const exprLabel = trimmed.startsWith("(") && trimmed.endsWith(")")
      ? trimmed.slice(1, -1).trim()
      : trimmed;
    const parts = exprLabel.split("/");
    if (parts.length !== 2) {
      return null;
    }
    const numerator = parts[0].trim();
    const denominator = parts[1].trim();
    if (!numerator || !denominator) {
      return null;
    }
    return buildExpressionRichMarkup(`${numerator} / ${denominator}`, valueText);
  }


  function buildPlainTextRichMarkup(text: string) {
    if (typeof text !== "string" || text.length === 0) {
      return null;
    }
    return `<H<Tx${text
      .split("&").join("＆")
      .split("<").join("＜")
      .split(">").join("＞")
      .split("*").join("\u00b7")}>>`;
  }


  function escapeRichText(text: string) {
    return String(text)
      .split("&").join("＆")
      .split("<").join("＜")
      .split(">").join("＞")
      .split("*").join("\u00b7");
  }


  function replaceRichMarkupPathValues(markup: string | null | undefined, valuesBySlot: Map<number, string>) {
    if (typeof markup !== "string" || valuesBySlot.size === 0) {
      return markup || null;
    }
    let output = "";
    let index = 0;
    while (index < markup.length) {
      if (!markup.startsWith("<?1x", index)) {
        output += markup[index];
        index += 1;
        continue;
      }
      const nodeStart = index;
      let nameEnd = index + 4;
      while (nameEnd < markup.length && markup[nameEnd] !== "<" && markup[nameEnd] !== ">") {
        nameEnd += 1;
      }
      const slotText = markup.slice(index + 4, nameEnd);
      const slot = /^\d+$/.test(slotText)
        ? Number(slotText)
        : (/^B\d+$/.test(slotText) ? Number(slotText.slice(1)) : NaN);
      const replacement = valuesBySlot.get(slot);
      if (replacement === undefined || markup[nameEnd] !== "<") {
        output += markup.slice(nodeStart, nameEnd);
        index = nameEnd;
        continue;
      }
      let depth = 1;
      let end = nameEnd;
      while (end < markup.length) {
        if (markup[end] === "<") {
          depth += 1;
        } else if (markup[end] === ">") {
          depth -= 1;
          if (depth === 0) {
            end += 1;
            break;
          }
        }
        end += 1;
      }
      if (depth !== 0) {
        output += markup.slice(nodeStart);
        return output;
      }
      output += `<?1x${slotText}<H<T1x${escapeRichText(replacement)}>>>`;
      index = end;
    }
    return output;
  }


  function replaceTemplateTextRanges(
    templateText: string,
    replacements: Array<{ line: number; start: number; end: number; valueText: string }>,
  ) {
    const lines = String(templateText).split("\n").map((line) => Array.from(line));
    [...replacements]
      .sort((left, right) => right.line - left.line || right.start - left.start)
      .forEach((replacement) => {
        const line = lines[replacement.line];
        if (!line) return;
        const start = Math.max(0, Math.min(line.length, replacement.start));
        const end = Math.max(start, Math.min(line.length, replacement.end));
        line.splice(start, end - start, ...Array.from(replacement.valueText));
      });
    return lines.map((line) => line.join("")).join("\n");
  }



  modules.dynamicsRichText = {
    buildExpressionRichMarkup,
    buildRatioValueRichMarkup,
    buildPlainTextRichMarkup,
    replaceRichMarkupPathValues,
    replaceTemplateTextRanges,
  };
})();
