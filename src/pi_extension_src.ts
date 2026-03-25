import { spawnSync } from "node:child_process";
import { writeFileSync, readFileSync, unlinkSync, rmdirSync, mkdtempSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import { DynamicBorder } from "@mariozechner/pi-coding-agent";
import { Container, SelectList, Text, type SelectItem } from "@mariozechner/pi-tui";

type ReviewTarget =
	| { type: "uncommitted" }
	| { type: "baseBranch"; branch: string; mergeBase: string }
	| { type: "commit"; sha: string };

let pi: ExtensionAPI;

export default function (apiRef: ExtensionAPI) {
	pi = apiRef;

	pi.registerCommand("pdiff", {
		description: "Interactive diff review with vim motions",
		handler: async (_args, ctx) => {
			const target = await showTargetSelector(ctx);
			if (!target) return;

			const diff = await getDiff(target, ctx);
			if (!diff || !diff.trim()) {
				ctx.ui.notify("No diff to review.", "warning");
				return;
			}

			const result = runPdiff(diff, ctx);

			if (!result || !result.trim() || result.includes("No comments.")) {
				ctx.ui.notify("No review comments.", "info");
				return;
			}

			pi.sendUserMessage(result);
		},
	});
}

async function showTargetSelector(ctx: ExtensionContext): Promise<ReviewTarget | null> {
	const items: SelectItem[] = [
		{ value: "uncommitted", label: "Uncommitted changes", description: "all changes vs HEAD" },
		{ value: "baseBranch", label: "Against base branch", description: "diff against another branch" },
		{ value: "commit", label: "Specific commit", description: "review a single commit" },
	];

	const result = await ctx.ui.custom<string | null>((tui, theme, _kb, done) => {
		const container = new Container();
		container.addChild(new DynamicBorder((str: string) => theme.fg("accent", str)));
		container.addChild(new Text(theme.fg("accent", theme.bold("pdiff — Select review target"))));

		const selectList = new SelectList(items, items.length, {
			selectedPrefix: (text: string) => theme.fg("accent", text),
			selectedText: (text: string) => theme.fg("accent", text),
			description: (text: string) => theme.fg("muted", text),
		});

		selectList.onSelect = (item: SelectItem) => done(item.value);
		selectList.onCancel = () => done(null);

		container.addChild(selectList);
		container.addChild(new DynamicBorder((str: string) => theme.fg("accent", str)));

		return {
			render(width: number) { return container.render(width); },
			invalidate() { container.invalidate(); },
			handleInput(data: string) { selectList.handleInput(data); tui.requestRender(); },
		};
	});

	if (!result) return null;

	switch (result) {
		case "uncommitted":
			return { type: "uncommitted" };

		case "baseBranch": {
			const branch = await ctx.ui.input("Enter base branch name (e.g. main):");
			if (!branch?.trim()) return null;

			const { stdout: mergeBase, code } = await pi.exec("git", [
				"merge-base", "HEAD", branch.trim(),
			]);
			if (code !== 0) {
				ctx.ui.notify(`Could not find merge base with ${branch.trim()}`, "error");
				return null;
			}
			return { type: "baseBranch", branch: branch.trim(), mergeBase: mergeBase.trim() };
		}

		case "commit": {
			const { stdout: logOutput, code } = await pi.exec("git", [
				"log", "--oneline", "-n", "20",
			]);
			if (code !== 0 || !logOutput.trim()) {
				ctx.ui.notify("Could not read git log.", "error");
				return null;
			}

			const commitLines = logOutput.trim().split("\n").map((l: string) => l.trim());
			const selected = await ctx.ui.select("Select commit to review:", commitLines);
			if (!selected) return null;
			const sha = selected.split(" ")[0];
			return { type: "commit", sha };
		}

		default:
			return null;
	}
}

async function getDiff(target: ReviewTarget, ctx: ExtensionContext): Promise<string | null> {
	let args: string[];

	switch (target.type) {
		case "uncommitted": {
			const { stdout: untracked } = await pi.exec("git", ["ls-files", "--others", "--exclude-standard"]);
			if (untracked?.trim()) {
				ctx.ui.notify("Untracked files skipped. Run 'git add' to include them.", "warning");
			}

			const { code: headCheck } = await pi.exec("git", ["rev-parse", "--verify", "HEAD"]);
			if (headCheck === 0) {
				args = ["diff", "HEAD"];
			} else {
				const { stdout: diff } = await pi.exec("git", ["diff", "--cached"]);
				if (!diff?.trim()) {
					ctx.ui.notify("No staged changes. Run 'git add' first.", "warning");
					return null;
				}
				return diff;
			}
			break;
		}
		case "baseBranch":
			// Compare committed state only, not working tree
			args = ["diff", target.mergeBase, "HEAD"];
			break;
		case "commit": {
			// For merge commits: diff against first parent only
			// For root commits: diff-tree --root shows the full tree as additions
			// For regular commits: diff against parent
			const { stdout: parentOut } = await pi.exec("git", ["rev-parse", "--verify", `${target.sha}^1`]);
			if (parentOut?.trim()) {
				args = ["diff", parentOut.trim(), target.sha];
			} else {
				args = ["diff-tree", "-p", "--root", target.sha];
			}
			break;
		}
	}

	const { stdout, code } = await pi.exec("git", args);
	if (code !== 0) return null;
	return stdout;
}

function runPdiff(diff: string, ctx: ExtensionContext): string | null {
	const dir = mkdtempSync(join(tmpdir(), "pdiff-"));
	const inputPath = join(dir, "input.diff");
	const outputPath = join(dir, "result.md");

	try {
		writeFileSync(inputPath, diff);

		const result = spawnSync("pdiff", [
			"--input", inputPath,
			"--output", outputPath,
		], {
			stdio: "inherit",
		});

		if (result.status !== 0) {
			ctx.ui.notify("pdiff exited with an error.", "warning");
			return null;
		}

		try {
			return readFileSync(outputPath, "utf-8");
		} catch {
			return null;
		}
	} finally {
		try { unlinkSync(inputPath); } catch {}
		try { unlinkSync(outputPath); } catch {}
		try { rmdirSync(dir); } catch {}
	}
}
