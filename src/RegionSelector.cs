using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace MultiViewerRadio
{
    public class RegionSelectionForm : Form
    {
        private enum DragMode { None, Draw, Move, NW, N, NE, E, SE, S, SW, W }
        private readonly Rectangle desktopBounds;
        private readonly Bitmap snapshot;
        private readonly Font labelFont = new Font("Microsoft YaHei UI", 10.5f, FontStyle.Bold);
        private readonly Font uiFont = new Font("Microsoft YaHei UI", 10f);
        private readonly Color accent = Color.FromArgb(45, 199, 245);
        private Rectangle selection;
        private Rectangle dragOrigin;
        private Point dragStart;
        private Point pointer;
        private DragMode dragMode;
        private bool resourcesDisposed;
        private const int MinimumWidth = 50;
        private const int MinimumHeight = 20;

        // The form owns the supplied snapshot. This constructor also allows
        // interaction checks without capturing or opening a desktop window.
        public RegionSelectionForm(Rectangle screenBounds, Rectangle initialBounds, Bitmap background)
        {
            desktopBounds = screenBounds;
            snapshot = background;
            FormBorderStyle = FormBorderStyle.None;
            StartPosition = FormStartPosition.Manual;
            AutoScaleMode = AutoScaleMode.None;
            Bounds = screenBounds;
            TopMost = true;
            ShowInTaskbar = false;
            KeyPreview = true;
            DoubleBuffered = true;
            Text = "框选字幕区域";
            Cursor = Cursors.Cross;
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint |
                     ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw, true);
            Rectangle initial = Rectangle.Intersect(initialBounds, screenBounds);
            if (initial.Width >= MinimumWidth && initial.Height >= MinimumHeight)
                selection = new Rectangle(initial.X - screenBounds.X, initial.Y - screenBounds.Y,
                                          initial.Width, initial.Height);
        }

        public static RegionSelectionForm CreateForDesktop(Rectangle initialBounds)
        {
            Rectangle screen = SystemInformation.VirtualScreen;
            Bitmap background = new Bitmap(screen.Width, screen.Height);
            try
            {
                using (Graphics graphics = Graphics.FromImage(background))
                    graphics.CopyFromScreen(screen.Location, Point.Empty, screen.Size, CopyPixelOperation.SourceCopy);
                return new RegionSelectionForm(screen, initialBounds, background);
            }
            catch { background.Dispose(); throw; }
        }

        public Rectangle SelectedScreenBounds
        {
            get { return new Rectangle(selection.X + desktopBounds.X, selection.Y + desktopBounds.Y,
                                       selection.Width, selection.Height); }
        }

        private bool HasSelection
        {
            get { return selection.Width >= MinimumWidth && selection.Height >= MinimumHeight; }
        }

        private Point ClampPoint(Point point)
        {
            return new Point(Math.Max(0, Math.Min(ClientSize.Width, point.X)),
                             Math.Max(0, Math.Min(ClientSize.Height, point.Y)));
        }

        private Rectangle ToolbarBounds()
        {
            int width = Math.Min(300, ClientSize.Width - 16);
            int x = Math.Max(8, Math.Min(selection.Left, ClientSize.Width - width - 8));
            int y = selection.Bottom + 14;
            if (y + 44 > ClientSize.Height - 8) y = selection.Top - 58;
            y = Math.Max(8, Math.Min(y, ClientSize.Height - 52));
            return new Rectangle(x, y, width, 44);
        }

        private Rectangle[] ToolbarButtons()
        {
            Rectangle bar = ToolbarBounds();
            return new[] {
                new Rectangle(bar.X + 6, bar.Y + 6, 118, 32),
                new Rectangle(bar.X + 130, bar.Y + 6, 66, 32),
                new Rectangle(bar.X + 202, bar.Y + 6, 92, 32)
            };
        }

        private Point[] HandlePoints()
        {
            int middleX = selection.Left + selection.Width / 2;
            int middleY = selection.Top + selection.Height / 2;
            return new[] {
                new Point(selection.Left, selection.Top), new Point(middleX, selection.Top),
                new Point(selection.Right, selection.Top), new Point(selection.Right, middleY),
                new Point(selection.Right, selection.Bottom), new Point(middleX, selection.Bottom),
                new Point(selection.Left, selection.Bottom), new Point(selection.Left, middleY)
            };
        }

        private DragMode HitSelection(Point point)
        {
            if (!HasSelection) return DragMode.Draw;
            Point[] handles = HandlePoints();
            DragMode[] modes = { DragMode.NW, DragMode.N, DragMode.NE, DragMode.E,
                                 DragMode.SE, DragMode.S, DragMode.SW, DragMode.W };
            // Corners take priority when a narrow region places handles close together.
            int[] order = { 0, 2, 4, 6, 1, 3, 5, 7 };
            foreach (int i in order)
                if (Math.Abs(point.X - handles[i].X) <= 9 && Math.Abs(point.Y - handles[i].Y) <= 9)
                    return modes[i];
            return selection.Contains(point) ? DragMode.Move : DragMode.Draw;
        }

        private void AcceptSelection()
        {
            if (!HasSelection) return;
            DialogResult = DialogResult.OK;
            Close();
        }

        private void CancelSelection()
        {
            DialogResult = DialogResult.Cancel;
            Close();
        }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            base.OnMouseDown(e);
            if (e.Button == MouseButtons.Right) { CancelSelection(); return; }
            if (e.Button != MouseButtons.Left) return;
            if (HasSelection)
            {
                Rectangle[] buttons = ToolbarButtons();
                if (buttons[0].Contains(e.Location)) { AcceptSelection(); return; }
                if (buttons[1].Contains(e.Location)) { selection = Rectangle.Empty; Invalidate(); return; }
                if (buttons[2].Contains(e.Location)) { CancelSelection(); return; }
                if (ToolbarBounds().Contains(e.Location)) return;
            }
            dragStart = ClampPoint(e.Location);
            dragOrigin = selection;
            dragMode = HitSelection(e.Location);
            if (dragMode == DragMode.Draw) selection = new Rectangle(dragStart, Size.Empty);
            Capture = true;
            Invalidate();
        }

        protected override void OnMouseMove(MouseEventArgs e)
        {
            base.OnMouseMove(e);
            Point previousPointer = pointer;
            pointer = e.Location;
            if (dragMode == DragMode.None)
            {
                SetPointerCursor();
                if (HasSelection)
                {
                    Rectangle bar = ToolbarBounds();
                    if (bar.Contains(previousPointer) || bar.Contains(pointer)) Invalidate(bar);
                }
                return;
            }
            Point current = ClampPoint(e.Location);
            int dx = current.X - dragStart.X;
            int dy = current.Y - dragStart.Y;
            if (dragMode == DragMode.Draw)
            {
                selection = Rectangle.FromLTRB(Math.Min(current.X, dragStart.X), Math.Min(current.Y, dragStart.Y),
                                               Math.Max(current.X, dragStart.X), Math.Max(current.Y, dragStart.Y));
            }
            else if (dragMode == DragMode.Move)
            {
                selection = new Rectangle(Math.Max(0, Math.Min(ClientSize.Width - dragOrigin.Width, dragOrigin.X + dx)),
                                          Math.Max(0, Math.Min(ClientSize.Height - dragOrigin.Height, dragOrigin.Y + dy)),
                                          dragOrigin.Width, dragOrigin.Height);
            }
            else if (dragMode != DragMode.None)
            {
                int left = dragOrigin.Left, right = dragOrigin.Right, top = dragOrigin.Top, bottom = dragOrigin.Bottom;
                if (dragMode == DragMode.NW || dragMode == DragMode.W || dragMode == DragMode.SW)
                    left = Math.Max(0, Math.Min(right - MinimumWidth, dragOrigin.Left + dx));
                if (dragMode == DragMode.NE || dragMode == DragMode.E || dragMode == DragMode.SE)
                    right = Math.Min(ClientSize.Width, Math.Max(left + MinimumWidth, dragOrigin.Right + dx));
                if (dragMode == DragMode.NW || dragMode == DragMode.N || dragMode == DragMode.NE)
                    top = Math.Max(0, Math.Min(bottom - MinimumHeight, dragOrigin.Top + dy));
                if (dragMode == DragMode.SW || dragMode == DragMode.S || dragMode == DragMode.SE)
                    bottom = Math.Min(ClientSize.Height, Math.Max(top + MinimumHeight, dragOrigin.Bottom + dy));
                selection = Rectangle.FromLTRB(left, top, right, bottom);
            }
            SetPointerCursor();
            Invalidate();
        }

        private void SetPointerCursor()
        {
            if (dragMode == DragMode.None && HasSelection && ToolbarBounds().Contains(pointer))
            { Cursor = Cursors.Hand; return; }
            DragMode mode = dragMode == DragMode.None ? HitSelection(pointer) : dragMode;
            switch (mode)
            {
                case DragMode.Move: Cursor = Cursors.SizeAll; break;
                case DragMode.NW: case DragMode.SE: Cursor = Cursors.SizeNWSE; break;
                case DragMode.NE: case DragMode.SW: Cursor = Cursors.SizeNESW; break;
                case DragMode.N: case DragMode.S: Cursor = Cursors.SizeNS; break;
                case DragMode.E: case DragMode.W: Cursor = Cursors.SizeWE; break;
                default: Cursor = Cursors.Cross; break;
            }
        }

        protected override void OnMouseUp(MouseEventArgs e)
        {
            base.OnMouseUp(e);
            if (e.Button != MouseButtons.Left) return;
            if (dragMode == DragMode.Draw && !HasSelection) selection = Rectangle.Empty;
            dragMode = DragMode.None;
            Capture = false;
            SetPointerCursor();
            Invalidate();
        }

        protected override void OnMouseDoubleClick(MouseEventArgs e)
        {
            base.OnMouseDoubleClick(e);
            if (e.Button == MouseButtons.Left && selection.Contains(e.Location)) AcceptSelection();
        }

        protected override bool ProcessCmdKey(ref Message msg, Keys keyData)
        {
            Keys key = keyData & Keys.KeyCode;
            if (key == Keys.Escape) { CancelSelection(); return true; }
            if (key == Keys.Enter) { AcceptSelection(); return true; }
            if (key == Keys.R) { selection = Rectangle.Empty; Invalidate(); return true; }
            if (HasSelection && (key == Keys.Left || key == Keys.Right || key == Keys.Up || key == Keys.Down))
            {
                int step = (keyData & Keys.Shift) == Keys.Shift ? 10 : 1;
                int x = selection.X + (key == Keys.Left ? -step : key == Keys.Right ? step : 0);
                int y = selection.Y + (key == Keys.Up ? -step : key == Keys.Down ? step : 0);
                selection.Location = new Point(Math.Max(0, Math.Min(ClientSize.Width - selection.Width, x)),
                                               Math.Max(0, Math.Min(ClientSize.Height - selection.Height, y)));
                Invalidate(); return true;
            }
            return base.ProcessCmdKey(ref msg, keyData);
        }

        private static GraphicsPath RoundedPath(Rectangle rectangle, int radius)
        {
            GraphicsPath path = new GraphicsPath();
            int diameter = radius * 2;
            path.AddArc(rectangle.Left, rectangle.Top, diameter, diameter, 180, 90);
            path.AddArc(rectangle.Right - diameter, rectangle.Top, diameter, diameter, 270, 90);
            path.AddArc(rectangle.Right - diameter, rectangle.Bottom - diameter, diameter, diameter, 0, 90);
            path.AddArc(rectangle.Left, rectangle.Bottom - diameter, diameter, diameter, 90, 90);
            path.CloseFigure(); return path;
        }

        private static void FillPill(Graphics graphics, Rectangle rectangle, Color color, int radius)
        {
            using (GraphicsPath path = RoundedPath(rectangle, radius))
            using (Brush brush = new SolidBrush(color)) graphics.FillPath(brush, path);
        }

        private void DrawCenteredText(Graphics graphics, string text, Rectangle rectangle, Color color)
        {
            using (Brush brush = new SolidBrush(color))
            using (StringFormat format = new StringFormat { Alignment = StringAlignment.Center, LineAlignment = StringAlignment.Center })
                graphics.DrawString(text, uiFont, brush, rectangle, format);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics graphics = e.Graphics;
            graphics.DrawImageUnscaled(snapshot, Point.Empty);
            graphics.SmoothingMode = SmoothingMode.AntiAlias;
            using (Region outside = new Region(ClientRectangle))
            using (Brush shade = new SolidBrush(Color.FromArgb(145, 8, 14, 22)))
            {
                if (selection.Width > 0 && selection.Height > 0) outside.Exclude(selection);
                graphics.FillRegion(shade, outside);
            }
            if (selection.Width > 0 && selection.Height > 0)
            {
                using (Pen shadow = new Pen(Color.FromArgb(65, 0, 0, 0), 6)) graphics.DrawRectangle(shadow, selection);
                using (Pen line = new Pen(accent, 2)) graphics.DrawRectangle(line, selection);
                foreach (Point handle in HandlePoints())
                {
                    using (Brush fill = new SolidBrush(accent)) graphics.FillEllipse(fill, handle.X - 5, handle.Y - 5, 10, 10);
                    using (Pen rim = new Pen(Color.White, 1.5f)) graphics.DrawEllipse(rim, handle.X - 5, handle.Y - 5, 10, 10);
                }
                Rectangle screen = SelectedScreenBounds;
                string label = String.Format("{0}, {1}   {2} × {3} px", screen.X, screen.Y, screen.Width, screen.Height);
                int badgeWidth = Math.Min(ClientSize.Width - 16, (int)graphics.MeasureString(label, labelFont).Width + 24);
                int badgeX = Math.Max(8, Math.Min(selection.Left, ClientSize.Width - badgeWidth - 8));
                int badgeY = selection.Top - 38;
                if (HasSelection && ToolbarBounds().Y < selection.Top) badgeY = selection.Top - 96;
                if (badgeY < 8) badgeY = Math.Min(ClientSize.Height - 40, selection.Top + 12);
                Rectangle badge = new Rectangle(badgeX, badgeY, badgeWidth, 30);
                FillPill(graphics, badge, Color.FromArgb(220, 21, 29, 40), 12);
                using (Brush brush = new SolidBrush(Color.White)) graphics.DrawString(label, labelFont, brush, badge.X + 12, badge.Y + 5);
                if (HasSelection && dragMode == DragMode.None)
                {
                    Rectangle bar = ToolbarBounds();
                    FillPill(graphics, bar, Color.FromArgb(240, 25, 34, 46), 12);
                    Rectangle[] buttons = ToolbarButtons();
                    string[] titles = { "确认  Enter", "重选", "取消  Esc" };
                    for (int i = 0; i < buttons.Length; i++)
                    {
                        Color background = i == 0 ? Color.FromArgb(24, 136, 180) : Color.FromArgb(35, 47, 63);
                        if (buttons[i].Contains(pointer)) background = i == 0 ? Color.FromArgb(31, 164, 208) : Color.FromArgb(53, 69, 89);
                        FillPill(graphics, buttons[i], background, 8);
                        DrawCenteredText(graphics, titles[i], buttons[i], Color.White);
                    }
                }
            }
            else
            {
                Rectangle hint = new Rectangle(Math.Max(8, (ClientSize.Width - 480) / 2), 24, Math.Min(480, ClientSize.Width - 16), 62);
                FillPill(graphics, hint, Color.FromArgb(235, 21, 29, 40), 14);
                DrawCenteredText(graphics, "拖动框选字幕区域  ·  Esc 取消", new Rectangle(hint.X, hint.Y + 5, hint.Width, 26), Color.White);
                DrawCenteredText(graphics, "请包含卡片正文与说话人姓名行", new Rectangle(hint.X, hint.Y + 31, hint.Width, 24), Color.FromArgb(152, 170, 191));
            }
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing && !resourcesDisposed)
            {
                resourcesDisposed = true;
                snapshot.Dispose(); labelFont.Dispose(); uiFont.Dispose();
            }
            base.Dispose(disposing);
        }
    }
}
