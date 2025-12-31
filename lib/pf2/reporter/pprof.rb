# frozen_string_literal: true

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
  end
end
