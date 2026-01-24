# frozen_string_literal: true

require 'stringio'
require 'zlib'

module Pf2
  module Reporter
    # A minimal Protobuf encoder.
    class Protobuf
      def initialize
        @data = +"".b
      end

      def to_bytes
        @data
      end

      # Wire types
      # Not implemented: I64, SGROUP, EGROUP, I32

      WIRETYPE_VARINT = 0
      WIRETYPE_LEN = 2

      def put_byte(b)
        @data << (b & 0xFF).chr(Encoding::BINARY)
      end

      def put_varint(val)
        # Emit 7-bit chunks
        # msb=1 for continuation
        while val > 0x7F
          put_byte((val & 0x7F) | 0x80)
          val >>= 7
        end
        put_byte(val & 0xFF)
      end

      def wire_varint(field, val)
        put_varint((field << 3) | WIRETYPE_VARINT) # tag
        put_varint(val)
      end

      def wire_len(field, bytes)
        put_varint((field << 3) | WIRETYPE_LEN) # tag
        put_varint(bytes.bytesize)
        @data << bytes.b
      end

      # ---

      # Scalar value types
      # Not implemented:
      #   double, float, uint32, sint32, sint64,
      #   fixed32, fixed64, sfixed32, sfixed64, bytes

      def int32(field, val)
        twos_comp = val & 0xFFFF_FFFF
        wire_varint(field, twos_comp)
      end

      def int64(field, val)
        twos_comp = val & 0xFFFF_FFFF_FFFF_FFFF
        wire_varint(field, twos_comp)
      end

      def uint64(field, val)
        wire_varint(field, val)
      end

      def bool(field, val)
        wire_varint(field, val ? 1 : 0)
      end

      def string(field, val)
        wire_len(field, val)
      end

      # ---

      def submessage(field, submsg)
        wire_len(field, submsg)
      end
    end

    PROTO_TAGS = {
      profile: {
        sample_type: 1,
        sample: 2,
        mapping: 3,
        location: 4,
        function: 5,
        string_table: 6,
        drop_frames: 7,
        keep_frames: 8,
        time_nanos: 9,
        duration_nanos: 10,
        period_type: 11,
        period: 12,
        comment: 13,
        default_sample_type: 14,
        doc_url: 15,
      },
      value_type: {
        type: 1,
        unit: 2,
      },
      sample: {
        location_id: 1,
        value: 2,
        label: 3,
      },
      label: {
        key: 1,
        str: 2,
        num: 3,
        num_unit: 4,
      },
      mapping: {
        id: 1,
        memory_start: 2,
        memory_limit: 3,
        file_offset: 4,
        filename: 5,
        build_id: 6,
        has_functions: 7,
        has_filenames: 8,
        has_line_numbers: 9,
        has_inline_frames: 10,
      },
      location: {
        id: 1,
        mapping_id: 2,
        address: 3,
        line: 4,
        is_folded: 5,
      },
      line: {
        function_id: 1,
        line: 2,
        column: 3,
      },
      function: {
        id: 1,
        name: 2,
        system_name: 3,
        filename: 4,
        start_line: 5,
      },
    }

    class Pprof
      def initialize(profile)
        @profile = profile
        @stack_weaver = StackWeaver.new(profile)

        @strings = []
        @string_id_table = {}
        @function_id_map = {}
        @location_id_map = {}

        string_index("") # index 0 is empty string
      end

      def emit
        pb = Protobuf.new

        pb.int64(PROTO_TAGS[:profile][:time_nanos], @profile[:start_timestamp_ns] || 0)
        pb.int64(PROTO_TAGS[:profile][:duration_nanos], @profile[:duration_ns] || 0)

        # Sample types
        pb.submessage(PROTO_TAGS[:profile][:sample_type], pb_value_type("samples", "count"))
        pb.submessage(PROTO_TAGS[:profile][:sample_type], pb_value_type("cpu", "nanoseconds"))

        # Period
        pb.submessage(PROTO_TAGS[:profile][:period_type], pb_value_type("cpu", "nanoseconds"))
        pb.int64(PROTO_TAGS[:profile][:period], inferred_period_ns)

        build_functions(pb)
        build_locations(pb)
        build_samples(pb)

        # String table
        @strings.each do |s|
          pb.string(PROTO_TAGS[:profile][:string_table], s)
        end

        buffer = StringIO.new
        Zlib::GzipWriter.wrap(buffer) { |gz| gz.write(pb.to_bytes) }
        buffer.string
      end

      private

      def string_index(str)
        if @string_id_table.key?(str)
          return @string_id_table[str]
        end

        id = @strings.length
        @strings << str
        @string_id_table[str] = id
        id
      end

      def pb_value_type(type, unit)
        pb = Protobuf.new
        pb.int64(PROTO_TAGS[:value_type][:type], string_index(type))
        pb.int64(PROTO_TAGS[:value_type][:unit], string_index(unit))
        pb.to_bytes
      end

      def pb_sample(location_ids, cpu_value_ns)
        pb = Protobuf.new
        # Equalivalent to:
        # location_ids.each do |loc_id|
        #   pb.uint64(PROTO_TAGS[:sample][:location_id], loc_id)
        # end
        # pb.int64(PROTO_TAGS[:sample][:value], 1) # samples
        # pb.int64(PROTO_TAGS[:sample][:value], cpu_value_ns) # cpu nanoseconds
        pack_repeated_uint64(pb, PROTO_TAGS[:sample][:location_id], location_ids)
        pack_repeated_int64(pb, PROTO_TAGS[:sample][:value], [1, cpu_value_ns])
        pb.to_bytes
      end

      def pack_repeated_uint64(pb, field_no, values)
        packed = Protobuf.new
        values.each { |v| packed.put_varint(v) }
        pb.wire_len(field_no, packed.to_bytes)
      end

      def pack_repeated_int64(pb, field_no, values)
        packed = Protobuf.new
        values.each { |v| packed.put_varint(v & 0xFFFF_FFFF_FFFF_FFFF) }
        pb.wire_len(field_no, packed.to_bytes)
      end

      # def pb_mapping(id, memory_start, memory_limit, file_offset, filename)
      #   pb = Protobuf.new
      #   pb.uint64(PROTO_TAGS[:mapping][:id], id)
      #   pb.uint64(PROTO_TAGS[:mapping][:memory_start], memory_start)
      #   pb.uint64(PROTO_TAGS[:mapping][:memory_limit], memory_limit)
      #   pb.uint64(PROTO_TAGS[:mapping][:file_offset], file_offset)
      #   pb.int64(PROTO_TAGS[:mapping][:filename], string_index(filename))
      #   pb.int64(PROTO_TAGS[:mapping][:build_id], 123456)
      #   pb.bool(PROTO_TAGS[:mapping][:has_functions], true)
      #   pb.bool(PROTO_TAGS[:mapping][:has_filenames], true)
      #   pb.bool(PROTO_TAGS[:mapping][:has_line_numbers], true)
      #   pb.bool(PROTO_TAGS[:mapping][:has_inline_frames], false)
      #   pb.to_bytes
      # end

      def pb_location(id, mapping_id, address, lines)
        pb = Protobuf.new
        pb.uint64(PROTO_TAGS[:location][:id], id)
        pb.uint64(PROTO_TAGS[:location][:mapping_id], mapping_id)
        pb.uint64(PROTO_TAGS[:location][:address], address)
        lines.each do |fn_id, lineno, column|
          pb.submessage(PROTO_TAGS[:location][:line], pb_line(fn_id, lineno, column || 0))
        end
        pb
      end

      def pb_line(function_id, line, column = 0)
        pb = Protobuf.new
        pb.uint64(PROTO_TAGS[:line][:function_id], function_id)
        pb.int32(PROTO_TAGS[:line][:line], line)
        pb.int32(PROTO_TAGS[:line][:column], column) if column != 0
        pb.to_bytes
      end

      def pb_function(id, name, system_name, filename, start_line)
        pb = Protobuf.new
        pb.uint64(PROTO_TAGS[:function][:id], id)
        pb.int64(PROTO_TAGS[:function][:name], string_index(name))
        pb.int64(PROTO_TAGS[:function][:system_name], string_index(system_name))
        pb.int64(PROTO_TAGS[:function][:filename], string_index(filename))
        pb.int64(PROTO_TAGS[:function][:start_line], start_line)
        pb.to_bytes
      end

      def build_functions(pb)
        @profile[:functions].each.with_index do |fn, idx|
          id = idx + 1
          @function_id_map[idx] = id
          pb.submessage(
            PROTO_TAGS[:profile][:function],
            pb_function(
              id,
              fn[:name] || "<unknown>",
              "",
              fn[:filename] || "<unknown>",
              fn[:start_lineno] || 0,
            )
          )
        end
      end

      def build_locations(pb)
        @profile[:locations].each.with_index do |loc, idx|
          id = idx + 1
          @location_id_map[idx] = id
          function_id = @function_id_map[loc[:function_index]] || 0
          pb.submessage(
            PROTO_TAGS[:profile][:location],
            pb_location(
              id,
              0, # mapping_id
              loc[:address] || 0,
              [[function_id, loc[:lineno] || 0]],
            ).to_bytes
          )
        end
      end

      def build_samples(pb)
        cpu_value_ns = inferred_sample_value_ns

        @profile[:samples].each do |sample|
          woven_stack = @stack_weaver.weave(sample[:stack], sample[:native_stack] || [])
          woven_stack.reverse! # leaf -> root for pprof
          location_ids = woven_stack.map { |idx| @location_id_map[idx] }.compact
          next if location_ids.empty?

          pb.submessage(
            PROTO_TAGS[:profile][:sample],
            pb_sample(location_ids, cpu_value_ns)
          )
        end
      end

      def inferred_period_ns
        value = inferred_sample_value_ns
        return value if value > 0

        9_000_000 # default 9 ms
      end

      def inferred_sample_value_ns
        samples = @profile[:samples]
        return 0 if samples.empty?

        duration = @profile[:duration_ns] || 0
        return 0 if duration == 0

        (duration.to_f / samples.length).round
      end
    end
  end
end
