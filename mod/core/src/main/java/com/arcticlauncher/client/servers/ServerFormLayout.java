package com.arcticlauncher.client.servers;

/** Bounds in scaled GUI pixels for the multiplayer server forms. */
final class ServerFormLayout {
	final Bounds panel;
	final Bounds title;
	final Bounds name;
	final Bounds address;
	final Bounds copy;
	final Bounds addToList;
	final Bounds submit;
	final Bounds cancel;

	private static final int PADDING = 10;
	private static final int MARGIN = 6;
	private static final int TITLE_HEIGHT = 14;
	private static final int COPY_WIDTH = 44;
	private static final int ADD_TO_LIST_WIDTH = 120;
	private static final int MIN_ADDRESS_WIDTH = 96;

	ServerFormLayout(int width, int height, boolean direct, int buttonHeight, int gap) {
		int inset = Math.min(MARGIN + PADDING, width / 4);
		int contentWidth = Math.min(direct ? 340 : 240, width - 2 * inset);
		int left = (width - contentWidth) / 2;
		int horizontalPadding = Math.min(PADDING, inset);
		int columnGap = Math.min(gap, contentWidth / 4);
		boolean stackActions = direct && contentWidth < MIN_ADDRESS_WIDTH + ADD_TO_LIST_WIDTH + COPY_WIDTH + 2 * gap;
		int rows = !direct || stackActions ? 3 : 2;
		int verticalPadding = Math.min(PADDING, height / 12);
		int titleHeight = Math.min(TITLE_HEIGHT, height / 4);
		int rowGap = Math.min(gap, height / 16);
		int rowHeight = Math.min(buttonHeight, (height - 2 * verticalPadding - titleHeight - rows * rowGap) / rows);
		int panelHeight = 2 * verticalPadding + titleHeight + rows * (rowHeight + rowGap);
		int panelTop = (height - panelHeight) / 2;
		panel = new Bounds(left - horizontalPadding, panelTop, contentWidth + 2 * horizontalPadding, panelHeight);
		title = new Bounds(left, panelTop + verticalPadding, contentWidth, titleHeight);

		int firstRow = title.bottom();
		int addressRow = direct ? firstRow : firstRow + rowHeight + rowGap;
		int actionsRow = stackActions ? addressRow + rowHeight + rowGap : addressRow;
		int copyWidth = Math.min(COPY_WIDTH, (contentWidth - columnGap) / 2);
		int addWidth = direct ? (stackActions ? contentWidth - columnGap - copyWidth : ADD_TO_LIST_WIDTH) : 0;
		int addressWidth = stackActions ? contentWidth
				: contentWidth - copyWidth - columnGap - (direct ? addWidth + columnGap : 0);
		name = direct ? null : new Bounds(left, firstRow, contentWidth, rowHeight);
		address = new Bounds(left, addressRow, addressWidth, rowHeight);
		copy = new Bounds(left + contentWidth - copyWidth, actionsRow, copyWidth, rowHeight);
		addToList = direct ? new Bounds(copy.x - columnGap - addWidth, actionsRow, addWidth, rowHeight) : null;

		int lastRow = actionsRow + rowHeight + rowGap;
		int submitWidth = (contentWidth - columnGap) / 2;
		submit = new Bounds(left, lastRow, submitWidth, rowHeight);
		cancel = new Bounds(left + submitWidth + columnGap, lastRow, contentWidth - submitWidth - columnGap, rowHeight);
	}

	static final class Bounds {
		final int x;
		final int y;
		final int width;
		final int height;

		Bounds(int x, int y, int width, int height) {
			this.x = x;
			this.y = y;
			this.width = width;
			this.height = height;
		}

		int right() {
			return x + width;
		}

		int bottom() {
			return y + height;
		}
	}
}
