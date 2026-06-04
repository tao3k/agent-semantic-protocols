"""Concept helpers for formatter-mediated compact AST tests."""

from __future__ import annotations

import ast
import re
from dataclasses import dataclass


@dataclass(frozen=True)
class AlignmentResult:
    status: str
    projection_mode: str
    failure_kind: str | None = None


@dataclass(frozen=True)
class CompactionReport:
    raw_chars: int
    compact_chars: int
    raw_prompt_tokens: int
    compact_prompt_tokens: int
    char_savings_percent: int
    prompt_token_savings_percent: int
    retained_agent_facts: tuple[str, ...]


def structural_fingerprint(source: str) -> str:
    tree = ast.parse(source)
    return ast.dump(tree, annotate_fields=True, include_attributes=False)


def prompt_token_estimate(text: str) -> int:
    """Stable proxy for prompt budget; not a model billing tokenizer."""
    token_like = re.findall(r"[A-Za-z_][A-Za-z0-9_]*|\d+|==|!=|<=|>=|->|:=|[^\s]", text)
    layout_cost = text.count("\n") + sum(
        1 for line in text.splitlines() if line.startswith((" ", "\t"))
    )
    return len(token_like) + layout_cost


def compact_args(args: ast.arguments) -> str:
    defaults_by_arg = {
        len(args.args) - len(args.defaults) + index: default
        for index, default in enumerate(args.defaults)
    }
    parts: list[str] = []
    for index, arg in enumerate(args.args):
        text = arg.arg
        if arg.annotation is not None:
            text += f": {ast.unparse(arg.annotation)}"
        if index in defaults_by_arg:
            text += f" = {ast.unparse(defaults_by_arg[index])}"
        parts.append(text)
    return ", ".join(parts)


def op_symbol(node: ast.AST) -> str:
    return {
        ast.Add: "+",
        ast.Sub: "-",
        ast.Mult: "*",
        ast.Div: "/",
        ast.Gt: ">",
        ast.Lt: "<",
        ast.GtE: ">=",
        ast.LtE: "<=",
        ast.Eq: "==",
        ast.NotEq: "!=",
    }.get(type(node), type(node).__name__)


def fact_expr(node: ast.AST | None) -> str:
    if node is None:
        return "none"
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Constant):
        return type(node.value).__name__.lower()
    if isinstance(node, ast.BinOp):
        return f"{fact_expr(node.left)}{op_symbol(node.op)}{fact_expr(node.right)}"
    if isinstance(node, ast.Compare) and len(node.ops) == 1 and len(node.comparators) == 1:
        return f"{fact_expr(node.left)}{op_symbol(node.ops[0])}{fact_expr(node.comparators[0])}"
    if isinstance(node, ast.Call):
        return f"{fact_expr(node.func)}/{len(node.args)}"
    if isinstance(node, ast.Attribute):
        return f"{fact_expr(node.value)}.{node.attr}"
    return type(node).__name__


def compact_projection(source: str) -> tuple[str, ...]:
    tree = ast.parse(source)
    visitor = ProjectionVisitor()
    visitor.visit(tree)
    return tuple(visitor.lines)


class ProjectionVisitor(ast.NodeVisitor):
    def __init__(self) -> None:
        self.lines: list[str] = []

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:  # noqa: N802
        returns = f" -> {ast.unparse(node.returns)}" if node.returns else ""
        self.lines.append(f"def {node.name}({compact_args(node.args)}){returns}")
        for child in node.body:
            self.visit(child)

    def visit_If(self, node: ast.If) -> None:  # noqa: N802
        self.lines.append(f"if {ast.unparse(node.test)}")
        for child in node.body:
            self.visit(child)
        for child in node.orelse:
            self.visit(child)

    def visit_Return(self, node: ast.Return) -> None:  # noqa: N802
        self.lines.append(f"return {ast.unparse(node.value)}")

    def visit_Expr(self, node: ast.Expr) -> None:  # noqa: N802
        if isinstance(node.value, ast.Call):
            self.lines.append(f"call {ast.unparse(node.value)}")


def ast_fact_projection(source: str) -> tuple[str, ...]:
    tree = ast.parse(source)
    visitor = FactVisitor()
    visitor.visit(tree)
    return tuple(visitor.lines)


class FactVisitor(ast.NodeVisitor):
    def __init__(self) -> None:
        self.lines: list[str] = []

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:  # noqa: N802
        returns = f"->{ast.unparse(node.returns)}" if node.returns else ""
        self.lines.append(f"F {node.name}/{len(node.args.args)}{returns}")
        for child in node.body:
            self.visit(child)

    def visit_If(self, node: ast.If) -> None:  # noqa: N802
        self.lines.append(f"B {fact_expr(node.test)}")
        for child in node.body:
            self.visit(child)
        for child in node.orelse:
            self.visit(child)

    def visit_Return(self, node: ast.Return) -> None:  # noqa: N802
        self.lines.append(f"R {fact_expr(node.value)}")

    def visit_Expr(self, node: ast.Expr) -> None:  # noqa: N802
        if isinstance(node.value, ast.Call):
            self.lines.append(f"E {fact_expr(node.value)}")


def retained_agent_facts(projection: tuple[str, ...]) -> tuple[str, ...]:
    facts: list[str] = []
    if any(line.startswith(("def ", "F ")) for line in projection):
        facts.append("declaration")
    if any(line.startswith(("if ", "B ")) for line in projection):
        facts.append("branch")
    if any(line.startswith(("call ", "E ")) for line in projection):
        facts.append("effect-call")
    if any(line.startswith(("return ", "R ")) for line in projection):
        facts.append("terminal-return")
    return tuple(facts)


def compaction_report(source: str) -> CompactionReport:
    compact_text = "\n".join(compact_projection(source))
    raw_prompt_tokens = prompt_token_estimate(source)
    compact_prompt_tokens = prompt_token_estimate(compact_text)
    raw_chars = len(source)
    compact_chars = len(compact_text)
    return CompactionReport(
        raw_chars=raw_chars,
        compact_chars=compact_chars,
        raw_prompt_tokens=raw_prompt_tokens,
        compact_prompt_tokens=compact_prompt_tokens,
        char_savings_percent=(raw_chars - compact_chars) * 100 // raw_chars,
        prompt_token_savings_percent=(raw_prompt_tokens - compact_prompt_tokens)
        * 100
        // raw_prompt_tokens,
        retained_agent_facts=retained_agent_facts(compact_projection(source)),
    )


def formatter_normalized_compact(
    original_source: str,
    formatted_source: str,
) -> AlignmentResult:
    original_fingerprint = structural_fingerprint(original_source)
    formatted_fingerprint = structural_fingerprint(formatted_source)

    if original_fingerprint != formatted_fingerprint:
        return AlignmentResult(
            status="failed",
            projection_mode="formatter-normalized",
            failure_kind="formatter-alignment-failed",
        )

    return AlignmentResult(
        status="ok",
        projection_mode="formatter-normalized",
    )
