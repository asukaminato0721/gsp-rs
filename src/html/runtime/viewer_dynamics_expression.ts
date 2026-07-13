(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  function evaluateExpr(expr: FunctionExprJson | FunctionAstJson, x: number, parameters: Map<string, number>): number | null {
    return window.GspRuntimeCore.evaluateExpr(expr, x, parameters);
  }


  function exprContainsPiAngle(expr: FunctionExprJson | FunctionAstJson | null | undefined): boolean {
    if (!expr || typeof expr !== "object") return false;
    if (expr.kind === "parsed") return exprContainsPiAngle(expr.expr);
    if (expr.kind === "pi-angle") return true;
    if (expr.kind === "unary") return exprContainsPiAngle(expr.expr);
    if (expr.kind === "binary") {
      return exprContainsPiAngle(expr.lhs) || exprContainsPiAngle(expr.rhs);
    }
    return false;
  }


  function formatExpr(expr: FunctionExprJson | FunctionAstJson, formatAxisNumber: (value: number) => string, variableLabel: string = "x"): string {
    if (expr.kind === "constant") return formatAxisNumber(expr.value);
    if (expr.kind === "identity") return variableLabel;
    if (expr.kind === "parsed") {
      return formatExprAst(expr.expr, formatAxisNumber, variableLabel, 0);
    }
    return "?";
  }


  function formatExprAst(expr: FunctionExprJson | FunctionAstJson | null | undefined, formatAxisNumber: (value: number) => string, variableLabel: string = "x", parentPrec: number = 0): string {
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


  function binaryPrecedence(op: string): { prec: number; rightAssoc: boolean } {
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


  function binaryOpText(op: string): string {
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
