// Runs the Gambatte hardware test ROMs on the reference emulator.
//
// Reads ROM paths on stdin and prints, for the DMG and the CGB model, one
// line per check: `model path PASS|FAIL expected actual`. This is the same
// format as `ceres-test-runner/examples/gambatte_runner.rs`, so the two
// outputs can be compared with `diff`. Only gambatte's public API is used.
//
// The expected result is encoded in the ROM's file name
// (`..._dmg08_out<hex>`, `..._cgb04c_out<hex>`, `..._dmg08_cgb04c_out<hex>`);
// only its leading hex digits count, upper-cased.

#include "gambatte.h"

#include <cctype>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <string>
#include <vector>

// The 8x8 hex digit glyphs (bit 7 = leftmost pixel, set = black).
static const unsigned char kGlyphs[16][8] = {
	{0x00, 0x7F, 0x41, 0x41, 0x41, 0x41, 0x41, 0x7F}, {0x00, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08},
	{0x00, 0x7F, 0x01, 0x01, 0x7F, 0x40, 0x40, 0x7F}, {0x00, 0x7F, 0x01, 0x01, 0x3F, 0x01, 0x01, 0x7F},
	{0x00, 0x41, 0x41, 0x41, 0x7F, 0x01, 0x01, 0x01}, {0x00, 0x7F, 0x40, 0x40, 0x7E, 0x01, 0x01, 0x7E},
	{0x00, 0x7F, 0x40, 0x40, 0x7F, 0x41, 0x41, 0x7F}, {0x00, 0x7F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10},
	{0x00, 0x3E, 0x41, 0x41, 0x3E, 0x41, 0x41, 0x3E}, {0x00, 0x7F, 0x41, 0x41, 0x7F, 0x01, 0x01, 0x7F},
	{0x00, 0x08, 0x22, 0x41, 0x7F, 0x41, 0x41, 0x41}, {0x00, 0x7E, 0x41, 0x41, 0x7E, 0x41, 0x41, 0x7E},
	{0x00, 0x3E, 0x41, 0x40, 0x40, 0x40, 0x41, 0x3E}, {0x00, 0x7E, 0x41, 0x41, 0x41, 0x41, 0x41, 0x7E},
	{0x00, 0x7F, 0x40, 0x40, 0x7F, 0x40, 0x40, 0x7F}, {0x00, 0x7F, 0x40, 0x40, 0x7F, 0x40, 0x40, 0x40},
};

static std::string expected(std::string const &stem, bool cgb) {
	char const *dmgKey = 0;
	char const *cgbKey = 0;
	if (stem.find("dmg08_cgb04c_out") != std::string::npos) {
		dmgKey = cgbKey = "dmg08_cgb04c_out";
	} else if (stem.find("dmg08_out") != std::string::npos) {
		dmgKey = "dmg08_out";
		if (stem.find("cgb04c_out") != std::string::npos)
			cgbKey = "cgb04c_out";
	} else if (stem.find("_out") != std::string::npos) {
		cgbKey = "_out";
	} else {
		return "";
	}

	char const *key = cgb ? cgbKey : dmgKey;
	if (!key)
		return "";

	std::string const out = stem.substr(stem.find(key) + std::strlen(key));
	if (out.compare(0, 5, "audio") == 0) // audio tests compare sound, not the screen
		return "";

	std::string hex;
	for (std::size_t i = 0; i < out.size() && std::isxdigit(static_cast<unsigned char>(out[i])); ++i)
		hex += static_cast<char>(std::toupper(static_cast<unsigned char>(out[i])));
	return hex;
}

static bool glyphMatches(std::vector<gambatte::uint_least32_t> const &frame, std::size_t cell, int digit) {
	for (int y = 0; y < 8; ++y) {
		for (int x = 0; x < 8; ++x) {
			gambatte::uint_least32_t const px = frame[y * 160 + cell * 8 + x] & 0xF8F8F8;
			bool const black = px == 0;
			bool const white = px == 0xF8F8F8;
			if (kGlyphs[digit][y] & (0x80 >> x) ? !black : !white)
				return false;
		}
	}
	return true;
}

static std::string screenText(std::vector<gambatte::uint_least32_t> const &frame, std::size_t digits) {
	std::string text;
	for (std::size_t cell = 0; cell < digits; ++cell) {
		char c = '?';
		for (int d = 0; d < 16; ++d) {
			if (glyphMatches(frame, cell, d)) {
				c = "0123456789ABCDEF"[d];
				break;
			}
		}
		text += c;
	}
	return text;
}

int main() {
	std::string path;
	while (std::getline(std::cin, path)) {
		std::size_t const slash = path.find_last_of('/');
		std::string const name = slash == std::string::npos ? path : path.substr(slash + 1);
		std::string const stem = name.substr(0, name.find_last_of('.'));

		for (int model = 0; model < 2; ++model) {
			bool const cgb = model == 1;
			std::string const want = expected(stem, cgb);
			if (want.empty())
				continue;

			gambatte::GB gb;
			unsigned const flags = (cgb ? gambatte::GB::CGB_MODE : 0) | gambatte::GB::NO_BIOS;
			if (gb.load(path, flags) != 0) {
				std::printf("%s %s LOADFAIL\n", cgb ? "cgb" : "dmg", path.c_str());
				continue;
			}

			// 15 frames from the post-boot state; the screen is read afterwards.
			std::vector<gambatte::uint_least32_t> frame(160 * 144);
			std::vector<gambatte::uint_least32_t> audio(35112 + 2064);
			for (int i = 0; i < 15; ++i) {
				std::size_t samples = 35112;
				gb.runFor(frame.data(), 160, audio.data(), samples);
			}

			std::string const got = screenText(frame, want.size());
			std::printf("%s %s %s %s %s\n", cgb ? "cgb" : "dmg", path.c_str(), got == want ? "PASS" : "FAIL",
			            want.c_str(), got.c_str());
		}
	}
}
