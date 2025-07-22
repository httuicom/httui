package requests

import (
	"encoding/json"
	"io"
	"net/http"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/gandarfh/httui/internal/storage"
	"github.com/gandarfh/httui/pkg/client"
	"github.com/gandarfh/httui/pkg/utils"
	"gorm.io/datatypes"
	"moul.io/http2curl"
)

type Result struct {
	Err      error
	Response any
	Loading  bool
}

func (m Model) Exec() tea.Cmd {
	return func() tea.Msg {
		request := m.Requests.Current

		url := utils.ReplaceByOperator(m.ResponsesRepo, request.Endpoint, m.Workspace.ID, m.Workspace.Environments.Data())
		res := client.Request(url, strings.ToUpper(string(request.Method)))

		rawbody, _ := request.Body.MarshalJSON()
		bodystring := utils.ReplaceByOperator(m.ResponsesRepo, string(rawbody), m.Workspace.ID, m.Workspace.Environments.Data())

		var body any
		if err := json.Unmarshal([]byte(bodystring), &body); err != nil {
			panic(err)
		}

		if _, ok := body.(map[string]any); ok {
			res.Body([]byte(bodystring))
		} else {
			res.Body(nil)
		}

		headers := utils.GetAllParentsHeaders(m.RequestsRepo, request.ParentID, request.Headers.Data())
		headers = utils.ProcessParamsOperators(m.ResponsesRepo, headers, m.Workspace.ID, m.Workspace.Environments.Data())

		for _, item := range headers {
			for k, v := range item {
				res.Header(k, v)
			}
		}

		params := utils.GetAllParentsParams(m.RequestsRepo, request.ParentID, request.QueryParams.Data())
		params = utils.ProcessParamsOperators(m.ResponsesRepo, params, m.Workspace.ID, m.Workspace.Environments.Data())

		for _, item := range params {
			for k, v := range item {
				res.Params(k, v)
			}
		}

		data, err := res.Exec()
		if err != nil {
			return Result{
				Response: datatypes.NewJSONType(err.Error()),
				Err:      err,
				Loading:  false,
			}
		}

		var response interface{}
		readbody, _ := io.ReadAll(data.Body)
		json.Unmarshal(readbody, &response)

		result := storage.Response{
			WorkspaceId:       m.Workspace.ID,
			Status:            data.Status,
			RequestId:         request.ID,
			RequestExternalId: request.ExternalId,
			Request:           datatypes.NewJSONType(request),
			Response:          datatypes.NewJSONType(response),
		}

		m.ResponsesRepo.Create(&result)

		return Result{
			Response: result,
		}
	}
}

func Curl(req *http.Request) string {
	command, _ := http2curl.GetCurlCommand(req)
	return command.String()
}
