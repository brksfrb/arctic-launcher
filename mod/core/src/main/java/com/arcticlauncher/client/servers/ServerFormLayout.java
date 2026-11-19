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

	ServerFormLayout(int width, int height, boolean direct) {
		int panelWidth = Math.min(direct ? 320 : 264, width - 2 * Math.min(12, width / 16));
		int panelHeight = Math.min(108, height - 2 * Math.min(12, height / 16));
		panel = new Bounds((width - panelWidth) / 2, (height - panelHeight) / 2, panelWidth, panelHeight);

		int padX = Math.min(12, panelWidth / 12);
		int padY = Math.min(10, panelHeight / 10);
		int contentWidth = panelWidth - 2 * padX;
		int gapX = Math.min(6, contentWidth / 8);
		int gapY = Math.min(6, panelHeight / 16);
		int rowSpace = panelHeight - 2 * padY - 3 * gapY;
		int titleHeight = Math.min(10, rowSpace / 4);
		int rowHeight = (rowSpace - titleHeight) / 3;

		int left = panel.x + padX;
		int top = panel.y + padY;
		int firstRow = top + titleHeight + gapY;
		int secondRow = firstRow + rowHeight + gapY;
		int lastRow = secondRow + rowHeight + gapY;
		int copyWidth = Math.min(56, (contentWidth - gapX) / 3);
		int remainingWidth = contentWidth - copyWidth - gapX;
		int submitWidth = (contentWidth - gapX) / 2;

		title = new Bounds(left, top, contentWidth, titleHeight);
		name = direct ? null : new Bounds(left, firstRow, contentWidth, rowHeight);
		address = direct ? new Bounds(left, firstRow, contentWidth, rowHeight)
				: new Bounds(left, secondRow, remainingWidth, rowHeight);
		addToList = direct ? new Bounds(left, secondRow, remainingWidth, rowHeight) : null;
		copy = new Bounds(left + remainingWidth + gapX, secondRow, copyWidth, rowHeight);
		submit = new Bounds(left, lastRow, submitWidth, rowHeight);
		cancel = new Bounds(left + submitWidth + gapX, lastRow, contentWidth - submitWidth - gapX, rowHeight);
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
