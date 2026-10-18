// SPDX-License-Identifier: MIT
//
// Recognises how a test ROM reports its result, and ends the simulation as
// soon as it is known. Compiled in next to dmg-sim (see the Makefile), it
// reaches into the chip by hierarchical names.
//
// Plusargs:
//   +TEST=<kind>         how the ROM reports its result:
//                          mooneye      registers B to L at LD B,B
//                          gbmicrotest  FF80 to FF82 in high RAM
//                          blargg       "Passed" or "Failed" on the serial port
//   +RESULT_FILE=<path>  where to write "pass ...", "fail ...", "timeout"
//                        (or "done" without +TEST) when the simulation ends
//   +SERIAL_FILE=<path>  where to write what the serial port sends
`default_nettype none

module probe_results;
	string test = "";
	string result_file = "";
	integer serial = 0;
	bit known = 0;

	initial begin
		string name;
		void'($value$plusargs("TEST=%s", test));
		void'($value$plusargs("RESULT_FILE=%s", result_file));
		if ($value$plusargs("SERIAL_FILE=%s", name))
			serial = $fopen(name, "w");
	end

	function automatic void write_result(input string result);
		integer f;
		$display("%0d ps: result: %s", longint'($realtime * 1000.0), result);
		if (result_file != "") begin
			f = $fopen(result_file, "w");
			$fdisplay(f, "%s", result);
			$fclose(f);
		end
	endfunction

	// Ends the simulation with a result.
	task automatic finish(input string result);
		known = 1;
		write_result(result);
		$finish;
	endtask

	// dmg-sim ran for the time it was given (+SECS).
	final if (!known) write_result(test == "" ? "done" : "timeout");

	// The CPU has read the entry point of the cartridge (0100): the boot ROM
	// is done, and the state of the chip is meaningful to the tests. The
	// address settles after /RD falls: any time during the read counts.
	bit booted = 0;

	always @(dmg_cpu_b_gameboy.nrd, dmg_cpu_b_gameboy.a)
		if (!booted && dmg_cpu_b_gameboy.nrd === 1'b0 && dmg_cpu_b_gameboy.a === 16'h0100) begin
			booted = 1;
			$display("%0d ps: the CPU reads the entry point 0100", longint'($realtime * 1000.0));
		end

	// Mooneye: the test ends with LD B,B (opcode 40), with the Fibonacci
	// numbers 3, 5, 8, 13, 21 and 34 in B, C, D, E, H and L if it passed.
	// The bits of the opcode register settle at different times, so 40 can
	// show for a moment between two other opcodes (from 00 to C3): it counts
	// if it holds for 200 ns, as an opcode does for at least one M-cycle.
	// Delays are in ns.
	initial begin
		#1;
		if (test == "mooneye") forever begin
			@(dmg_cpu_b_gameboy.dmg.cpu_inst.opcode);
			if (booted && dmg_cpu_b_gameboy.dmg.cpu_inst.opcode === 8'h40) begin
				#200;
				if (dmg_cpu_b_gameboy.dmg.cpu_inst.opcode === 8'h40) begin
					// Let the instruction finish.
					#2000;
					check_registers();
				end
			end
		end
	end

	task automatic check_registers;
		logic [7:0] b, c, d, e, h, l;
		b = dmg_cpu_b_gameboy.dmg.cpu_inst.reg_b;
		c = dmg_cpu_b_gameboy.dmg.cpu_inst.reg_c;
		d = dmg_cpu_b_gameboy.dmg.cpu_inst.reg_d;
		e = dmg_cpu_b_gameboy.dmg.cpu_inst.reg_e;
		h = dmg_cpu_b_gameboy.dmg.cpu_inst.reg_h;
		l = dmg_cpu_b_gameboy.dmg.cpu_inst.reg_l;
		if ({b, c, d, e, h, l} === 48'h0305080D1522)
			finish("pass");
		else
			finish($sformatf("fail B=%h C=%h D=%h E=%h H=%h L=%h", b, c, d, e, h, l));
	endtask

	// gbmicrotest: the test writes its result to FF80, the expected value
	// to FF81, and 01 (passed) or FF (failed) to FF82, all in high RAM, which
	// stores its bytes with the bits of the data bus reversed. RAM is written
	// as long as /WR is low, and the bus holds the precharged FF before the
	// CPU drives the byte: a result counts once it holds for 2 us.
	function automatic logic [7:0] reversed(input logic [7:0] v);
		for (int i = 0; i < 8; i++)
			reversed[i] = v[7 - i];
	endfunction

	logic [7:0] ff80, ff81, ff82;
	assign ff80 = reversed(dmg_cpu_b_gameboy.dmg.high_ram_inst.sram_inst.mem[0]);
	assign ff81 = reversed(dmg_cpu_b_gameboy.dmg.high_ram_inst.sram_inst.mem[1]);
	assign ff82 = reversed(dmg_cpu_b_gameboy.dmg.high_ram_inst.sram_inst.mem[2]);

	initial begin
		#1;
		if (test == "gbmicrotest") forever begin
			#1000;
			if (ff82 === 8'h01 || ff82 === 8'hFF) begin
				#2000;
				if (ff82 === 8'h01)
					finish($sformatf("pass FF80=%h FF81=%h", ff80, ff81));
				else if (ff82 === 8'hFF)
					finish($sformatf("fail FF80=%h FF81=%h", ff80, ff81));
			end
		end
	end

	// The other end of the serial link: what the Game Boy sends with its
	// internal clock, a byte with the most significant bit first, sampled on
	// the rising edge of SCK. Blargg's tests print their result there.
	logic [7:0] byte_in;
	int bits = 0;
	string received = "";

	always @(posedge dmg_cpu_b_gameboy.sck) begin
		byte_in = {byte_in[6:0], dmg_cpu_b_gameboy.sout};
		bits++;
		if (bits == 8) begin
			bits = 0;
			if (serial)
				$fwrite(serial, "%c", byte_in);
			received = $sformatf("%s%c", received, byte_in);
			if (test == "blargg" && received.len() >= 6) begin
				if (received.substr(received.len() - 6, received.len() - 1) == "Passed")
					finish({"pass serial: ", received});
				else if (received.substr(received.len() - 6, received.len() - 1) == "Failed")
					finish({"fail serial: ", received});
			end
		end
	end
endmodule

`default_nettype wire
