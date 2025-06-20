package details

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
	"github.com/gandarfh/httui/pkg/truncate"
)

func (m Model) Title() string {
	titleWidth := m.Width - m.Width/3

	id := lipgloss.NewStyle().
		Width(titleWidth / 3).
		Bold(true).
		Render(truncate.String(fmt.Sprintf(" ID: %d", m.Request.ID), titleWidth/4))

	name := " Name: " + m.Request.Name

	if m.Cursor == CursorName {
		name = " Name: " + m.InputName.View()
	}

	title := title_style.Width(m.Width).Border(lipgloss.RoundedBorder()).Render(
		lipgloss.JoinHorizontal(
			lipgloss.Left,
			lipgloss.NewStyle().Width(titleWidth).Bold(true).Render(name),
			lipgloss.NewStyle().Width(titleWidth/6).String(),
			id,
		))

	return title
}
