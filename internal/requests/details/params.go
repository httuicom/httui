package details

import (
	"fmt"

	"github.com/charmbracelet/glamour"
	"github.com/charmbracelet/lipgloss"
	"github.com/gandarfh/httui/pkg/utils"
)

func (m Model) Params() string {
	paramsWidth := m.Width/3 - 2
	maxTextValueSize := paramsWidth / 4

	paramrenderer, _ := glamour.NewTermRenderer(
		glamour.WithStyles(styleConfig),
		glamour.WithWordWrap(paramsWidth),
	)

	rawparams := utils.GetAllParentsParams(m.RequestsRepo, m.Request.ParentID, m.Request.QueryParams.Data())
	jsonString := DataToString(rawparams, maxTextValueSize, m.Height/3-2)

	query, _ := paramrenderer.Render("```json\n" + jsonString + "\n  ```")

	query_box := lipgloss.NewStyle().Height(m.Height/2 - 2).Render(lipgloss.JoinVertical(
		lipgloss.Left,
		lipgloss.NewStyle().Bold(true).Render(fmt.Sprint(" Params:")),
		query,
	))

	return query_box
}
