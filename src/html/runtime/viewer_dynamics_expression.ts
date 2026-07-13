(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  function evaluateUnary(op: string, x: number, degrees: boolean = false) {
    const value = degrees ? x * Math.PI / 180 : x;
    switch (op) {
      case "sin": return Math.sin(value);
      case "cos": return Math.cos(value);
      case "tan": return Math.tan(value);
      case "abs": return Math.abs(x);
      case "sqrt": return x >= 0 ? Math.sqrt(x) : null;
      case "ln": return x > 0 ? Math.log(x) : null;
      case "log10": return x > 0 ? Math.log10(x) : null;
      case "sign": return x > 0 ? 1 : (x < 0 ? -1 : 0);
      case "round": return Math.round(x);
      case "trunc": return Math.trunc(x);
      default: return null;
    }
  }


  function evaluateExpr(expr: FunctionExprJson | FunctionAstJson, x: number, parameters: Map<string, number>) {
    if (expr.kind === "constant") return expr.value;
    if (expr.kind === "identity") return x;
    if (expr.kind !== "parsed") return null;
    return evaluateExprAst(expr.expr, x, parameters);
  }


  function evaluateExprAst(expr: FunctionExprJson | FunctionAstJson, x: number, parameters: Map<string, number>) {
    if (!expr || typeof expr !== "object") return null;
    switch (expr.kind) {
      case "variable":
        return x;
      case "constant":
        return expr.value;
      case "parameter":
        return parameters.get(expr.name) ?? expr.value;
      case "pi-angle":
        return 180;
      case "unary": {
        const inner = evaluateExprAst(expr.expr, x, parameters);
        return inner === null ? null : evaluateUnary(expr.op, inner, exprContainsPiAngle(expr.expr));
      }
      case "binary": {
        const lhs = evaluateExprAst(expr.lhs, x, parameters);
        const rhs = evaluateExprAst(expr.rhs, x, parameters);
        if (lhs === null || rhs === null) return null;
        const value = expr.op === "add"
          ? lhs + rhs
          : expr.op === "sub"
            ? lhs - rhs
          : expr.op === "mul"
            ? lhs * rhs
          : expr.op === "div"
            ? (Math.abs(rhs) >= 1e-9 ? lhs / rhs : null)
          : Math.pow(lhs, rhs);
        return value === null || !Number.isFinite(value) ? null : value;
      }
      default:
        return null;
    }
  }


  function exprContainsPiAngle(expr: FunctionExprJson | FunctionAstJson | null | undefined) {
    if (!expr || typeof expr !== "object") return false;
    if (expr.kind === "parsed") return exprContainsPiAngle(expr.expr);
    if (expr.kind === "pi-angle") return true;
    if (expr.kind === "unary") return exprContainsPiAngle(expr.expr);
    if (expr.kind === "binary") {
      return exprContainsPiAngle(expr.lhs) || exprContainsPiAngle(expr.rhs);
    }
    return false;
  }


  function formatExpr(expr: FunctionExprJson | FunctionAstJson, formatAxisNumber: (value: number) => string, variableLabel: string = "x") {
    if (expr.kind === "constant") return formatAxisNumber(expr.value);
    if (expr.kind === "identity") return variableLabel;
    if (expr.kind === "parsed") {
      return formatExprAst(expr.expr, formatAxisNumber, variableLabel, 0);
    }
    return "?";
  }


  function formatExprAst(expr: FunctionExprJson | FunctionAstJson | null | undefined, formatAxisNumber: (value: number) => string, variableLabel: string = "x", parentPrec: number = 0) {
    if (!expr || typeof expr !== "object") return "?";
    switch (expr.kind) {
      case "variable":
        return variableLabel;
      case "constant":
        return formatAxisNumber(expr.value);
      case "parameter":
        return expr.name;
      case "pi-angle":
        return "180";
      case "unary": {

        const inner = formatExprAst(expr.expr, formatAxisNumber, variableLabel, 0);
        if (expr.op === "abs") return `|${inner}|`;
        if (expr.op === "sqrt") {
          return expr.expr?.kind === "binary" ? `√(${inner})` : `√${inner}`;
        }
        return `${expr.op}(${inner})`;
      }
      case "binary": {
        const { prec, rightAssoc } = binaryPrecedence(expr.op);

        const left = formatExprAst(expr.lhs, formatAxisNumber, variableLabel, prec);

        const right = formatExprAst(
          expr.rhs,
          formatAxisNumber,
          variableLabel,
          prec + (rightAssoc ? 0 : 1),
        );

        const text = `${left}${binaryOpText(expr.op)}${right}`;
        return prec < parentPrec ? `(${text})` : text;
      }
      default:
        return "?";
    }
  }


  function binaryPrecedence(op: string) {
    switch (op) {
      case "add":
      case "sub":
        return { prec: 1, rightAssoc: false };
      case "mul":
      case "div":
        return { prec: 2, rightAssoc: false };
      case "pow":
        return { prec: 3, rightAssoc: true };
      default:
        return { prec: 0, rightAssoc: false };
    }
  }


  function binaryOpText(op: string) {
    switch (op) {
      case "add": return " + ";
      case "sub": return " - ";
      case "mul": return "*";
      case "div": return " / ";
      case "pow": return "^";
      default: return " ? ";
    }
  }



  modules.dynamicsExpression = { evaluateExpr, formatExpr, exprContainsPiAngle };
})();
