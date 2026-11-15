package com.arcticlauncher.client.servers;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.DataInput;
import java.io.DataInputStream;
import java.io.DataOutput;
import java.io.DataOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * The game's {@code servers.dat}: an uncompressed NBT file whose {@code servers} list holds one
 * compound per server ({@code name}, {@code ip}, {@code icon} as base64, {@code acceptTextures},
 * and since 1.19.3 {@code hidden} for direct connections it remembers). The same on every
 * version, so Arctic edits it itself; every tag is kept as read, ours or not.
 */
public final class ServersFile {
	private static final byte END = 0;
	private static final byte BYTE = 1;
	private static final byte SHORT = 2;
	private static final byte INT = 3;
	private static final byte LONG = 4;
	private static final byte FLOAT = 5;
	private static final byte DOUBLE = 6;
	private static final byte BYTE_ARRAY = 7;
	private static final byte STRING = 8;
	private static final byte LIST = 9;
	private static final byte COMPOUND = 10;
	private static final byte INT_ARRAY = 11;
	private static final byte LONG_ARRAY = 12;
	private static final int MAX_DEPTH = 64;

	/** A tag: its type and value (a compound is a map of tags, a list a {@link TagList}). */
	static final class Tag {
		final byte type;
		final Object value;

		Tag(byte type, Object value) {
			this.type = type;
			this.value = value;
		}
	}

	static final class TagList {
		final byte elementType;
		final List<Tag> items;

		TagList(byte elementType, List<Tag> items) {
			this.elementType = elementType;
			this.items = items;
		}
	}

	private final File file;
	/** Everything in the file; {@link #servers} is its {@code servers} list. */
	private Map<String, Tag> root = new LinkedHashMap<String, Tag>();
	private final List<Map<String, Tag>> servers = new ArrayList<Map<String, Tag>>();

	public ServersFile(File file) {
		this.file = file;
	}

	/** Read the file (a missing one is an empty list). */
	@SuppressWarnings("unchecked")
	public ServersFile load() throws IOException {
		servers.clear();
		root = new LinkedHashMap<String, Tag>();
		if (!file.isFile()) {
			return this;
		}
		try (DataInputStream in = new DataInputStream(new BufferedInputStream(new FileInputStream(file)))) {
			if (in.readByte() != COMPOUND) {
				throw new IOException("servers.dat isn't a compound");
			}
			in.readUTF();
			root = (Map<String, Tag>) read(in, COMPOUND, 0);
		}
		Tag list = root.get("servers");
		if (list != null && list.type == LIST) {
			java.util.Set<String> seen = new java.util.HashSet<String>();
			for (Tag t : ((TagList) list.value).items) {
				if (t.type == COMPOUND) {
					Map<String, Tag> server = (Map<String, Tag>) t.value;
					Tag hidden = server.get("hidden");
					boolean invisible = hidden != null && hidden.type == BYTE && (Byte) hidden.value != 0;
					if (invisible || seen.add(normalizeAddress(string(server, "ip")))) servers.add(server);
				}
			}
		}
		return this;
	}

	/** Write it back (the previous file is kept as {@code servers.dat_old}, as the game does). */
	public void save() throws IOException {
		List<Tag> items = new ArrayList<Tag>();
		for (Map<String, Tag> s : servers) {
			items.add(new Tag(COMPOUND, s));
		}
		root.put("servers", new Tag(LIST, new TagList(COMPOUND, items)));
		File part = new File(file.getPath() + ".arctic-part");
		try (DataOutputStream out = new DataOutputStream(new BufferedOutputStream(new FileOutputStream(part)))) {
			out.writeByte(COMPOUND);
			out.writeUTF("");
			write(out, COMPOUND, root);
		}
		if (file.isFile()) {
			Files.copy(file.toPath(), new File(file.getPath() + "_old").toPath(), StandardCopyOption.REPLACE_EXISTING);
		}
		Files.move(part.toPath(), file.toPath(), StandardCopyOption.REPLACE_EXISTING);
	}

	// ---- The list (hidden entries are kept but not shown, and don't count) ---------------

	private List<Integer> shown() {
		List<Integer> out = new ArrayList<Integer>();
		java.util.Set<String> seen = new java.util.HashSet<String>();
		for (int i = 0; i < servers.size(); i++) {
			Tag hidden = servers.get(i).get("hidden");
			if ((hidden == null || hidden.type != BYTE || (Byte) hidden.value == 0)
					&& seen.add(normalizeAddress(string(servers.get(i), "ip")))) {
				out.add(i);
			}
		}
		return out;
	}

	public int size() {
		return shown().size();
	}

	public String name(int index) {
		return string(servers.get(shown().get(index)), "name");
	}

	public String address(int index) {
		return string(servers.get(shown().get(index)), "ip");
	}

	/** The saved icon (a PNG), or null. */
	public byte[] icon(int index) {
		String b64 = string(servers.get(shown().get(index)), "icon");
		if (b64.isEmpty()) {
			return null;
		}
		try {
			return Base64.getMimeDecoder().decode(b64);
		} catch (IllegalArgumentException e) {
			return null;
		}
	}

	/** Server resource packs: true (accept), false (decline) or null (ask). */
	public Boolean acceptsPacks(int index) {
		Tag t = servers.get(shown().get(index)).get("acceptTextures");
		return t == null || t.type != BYTE ? null : (Byte) t.value != 0;
	}

	/** Move shown entry {@code from} to shown place {@code to}. */
	public void move(int from, int to) {
		List<Integer> shown = shown();
		Map<String, Tag> moving = servers.remove((int) shown.get(from));
		// Where the entry now at shown place `to` sits (after the removal).
		List<Integer> after = shown();
		int at;
		if (to >= after.size()) {
			at = after.isEmpty() ? servers.size() : after.get(after.size() - 1) + 1;
		} else {
			at = after.get(to);
		}
		servers.add(at, moving);
	}

	/** Address identity without DNS: case, trailing DNS dot and explicit default port don't distinguish rows. */
	public static String normalizeAddress(String address) {
		String a = address.trim().toLowerCase(java.util.Locale.ROOT);
		String host = a;
		String port = "25565";
		if (a.startsWith("[")) {
			int end = a.indexOf(']');
			if (end > 0) {
				host = a.substring(1, end);
				if (a.length() > end + 1 && a.charAt(end + 1) == ':') port = a.substring(end + 2);
			}
		} else if (a.indexOf(':') > 0 && a.indexOf(':') == a.lastIndexOf(':')) {
			host = a.substring(0, a.indexOf(':'));
			port = a.substring(a.indexOf(':') + 1);
		}
		while (host.endsWith(".")) host = host.substring(0, host.length() - 1);
		try { port = Integer.toString(Integer.parseInt(port)); } catch (NumberFormatException ignored) { }
		return "[" + host + "]:" + port;
	}

	public void add(String name, String address) {
		for (Map<String, Tag> existing : servers) {
			if (normalizeAddress(string(existing, "ip")).equals(normalizeAddress(address))) return;
		}
		Map<String, Tag> s = new LinkedHashMap<String, Tag>();
		s.put("ip", new Tag(STRING, address));
		s.put("name", new Tag(STRING, name));
		servers.add(s);
	}

	/** New name and address; the icon goes if the address changed (it was the old server's). */
	public void edit(int index, String name, String address) {
		Map<String, Tag> s = servers.get(shown().get(index));
		if (!address.equals(string(s, "ip"))) {
			s.remove("icon");
		}
		s.put("name", new Tag(STRING, name));
		s.put("ip", new Tag(STRING, address));
	}

	public void remove(int index) {
		String address = normalizeAddress(address(index));
		for (int i = servers.size() - 1; i >= 0; i--) {
			Tag hidden = servers.get(i).get("hidden");
			if ((hidden == null || hidden.type != BYTE || (Byte) hidden.value == 0)
					&& normalizeAddress(string(servers.get(i), "ip")).equals(address)) servers.remove(i);
		}
	}

	private static String string(Map<String, Tag> s, String key) {
		Tag t = s.get(key);
		return t != null && t.type == STRING ? (String) t.value : "";
	}

	// ---- NBT -----------------------------------------------------------------------------

	private static Object read(DataInput in, byte type, int depth) throws IOException {
		if (depth > MAX_DEPTH) {
			throw new IOException("servers.dat nests too deep");
		}
		switch (type) {
			case BYTE:
				return in.readByte();
			case SHORT:
				return in.readShort();
			case INT:
				return in.readInt();
			case LONG:
				return in.readLong();
			case FLOAT:
				return in.readFloat();
			case DOUBLE:
				return in.readDouble();
			case BYTE_ARRAY: {
				byte[] b = new byte[length(in)];
				in.readFully(b);
				return b;
			}
			case STRING:
				return in.readUTF();
			case LIST: {
				byte element = in.readByte();
				int n = length(in);
				List<Tag> items = new ArrayList<Tag>(Math.min(n, 1024));
				for (int i = 0; i < n; i++) {
					items.add(new Tag(element, read(in, element, depth + 1)));
				}
				return new TagList(element, items);
			}
			case COMPOUND: {
				Map<String, Tag> map = new LinkedHashMap<String, Tag>();
				while (true) {
					byte t = in.readByte();
					if (t == END) {
						return map;
					}
					String name = in.readUTF();
					map.put(name, new Tag(t, read(in, t, depth + 1)));
				}
			}
			case INT_ARRAY: {
				int[] a = new int[length(in)];
				for (int i = 0; i < a.length; i++) {
					a[i] = in.readInt();
				}
				return a;
			}
			case LONG_ARRAY: {
				long[] a = new long[length(in)];
				for (int i = 0; i < a.length; i++) {
					a[i] = in.readLong();
				}
				return a;
			}
			default:
				throw new IOException("unknown tag type " + type);
		}
	}

	private static int length(DataInput in) throws IOException {
		int n = in.readInt();
		if (n < 0 || n > (1 << 24)) {
			throw new IOException("bad length " + n);
		}
		return n;
	}

	@SuppressWarnings("unchecked")
	private static void write(DataOutput out, byte type, Object value) throws IOException {
		switch (type) {
			case BYTE:
				out.writeByte((Byte) value);
				break;
			case SHORT:
				out.writeShort((Short) value);
				break;
			case INT:
				out.writeInt((Integer) value);
				break;
			case LONG:
				out.writeLong((Long) value);
				break;
			case FLOAT:
				out.writeFloat((Float) value);
				break;
			case DOUBLE:
				out.writeDouble((Double) value);
				break;
			case BYTE_ARRAY: {
				byte[] b = (byte[]) value;
				out.writeInt(b.length);
				out.write(b);
				break;
			}
			case STRING:
				out.writeUTF((String) value);
				break;
			case LIST: {
				TagList list = (TagList) value;
				out.writeByte(list.items.isEmpty() && list.elementType == END ? END : list.elementType);
				out.writeInt(list.items.size());
				for (Tag t : list.items) {
					write(out, list.elementType, t.value);
				}
				break;
			}
			case COMPOUND:
				for (Map.Entry<String, Tag> e : ((Map<String, Tag>) value).entrySet()) {
					out.writeByte(e.getValue().type);
					out.writeUTF(e.getKey());
					write(out, e.getValue().type, e.getValue().value);
				}
				out.writeByte(END);
				break;
			case INT_ARRAY: {
				int[] a = (int[]) value;
				out.writeInt(a.length);
				for (int v : a) {
					out.writeInt(v);
				}
				break;
			}
			case LONG_ARRAY: {
				long[] a = (long[]) value;
				out.writeInt(a.length);
				for (long v : a) {
					out.writeLong(v);
				}
				break;
			}
			default:
				throw new IOException("unknown tag type " + type);
		}
	}
}
