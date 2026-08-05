import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { api } from "../../api";

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
});

describe("api.searchSessions", () => {
  it("invokes search_sessions with correct params", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    await api.searchSessions("rust", undefined, { starred_only: false });
    expect(mockInvoke).toHaveBeenCalledWith("search_sessions", {
      query: "rust",
      limit: undefined,
      filters: { starred_only: false },
    });
  });
});

describe("api.confirmDelete", () => {
  it("calls confirm with the given message", async () => {
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    const mockConfirm = vi.mocked(confirm);
    mockConfirm.mockResolvedValueOnce(true);
    const result = await api.confirmDelete("テスト削除");
    expect(result).toBe(true);
    expect(mockConfirm).toHaveBeenCalledWith("テスト削除", {
      title: "削除の確認",
      kind: "warning",
    });
  });
});

describe("api.getTagMetadata", () => {
  it("invokes get_tag_metadata", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    const result = await api.getTagMetadata();
    expect(mockInvoke).toHaveBeenCalledWith("get_tag_metadata");
    expect(result).toEqual([]);
  });
});
