const std = @import("std");
const XxHash3 = std.hash.XxHash3;

var hasher = XxHash3.init(0);

export var buffer: [65536]u8 = undefined;

export fn init(seed: u64) void {
    hasher = XxHash3.init(seed);
}

export fn update(data: [*]const u8, len: usize) void {
    hasher.update(data[0..len]);
}

export fn digest() u64 {
    return hasher.final();
}
