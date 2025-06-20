package client

import (
	"bytes"
	"encoding/json"
	"io"
	"net/http"
	"time"
)

type header struct {
	key   string
	value string
}

type params struct {
	key   string
	value string
}

type client struct {
	Url     string
	method  string
	body    io.Reader
	params  []*params
	headers []*header
	cookies []*http.Cookie
}

func (c *client) Header(key string, value string) *client {
	c.headers = append(c.headers, &header{
		key:   key,
		value: value,
	})

	return c
}

func (c *client) AddCookie(cookie *http.Cookie) *client {
	c.cookies = append(c.cookies, cookie)

	return c
}

func (c *client) Params(key string, value string) *client {
	c.params = append(c.params, &params{key: key, value: value})
	return c
}

func (c *client) Body(b []byte) *client {
	c.body = bytes.NewBuffer(b)
	return c
}

func (c *client) Exec() (*http.Response, error) {
	request, err := http.NewRequest(c.method, c.Url, c.body)

	if err != nil {
		return nil, err
	}

	q := request.URL.Query()
	for _, item := range c.params {
		q.Add(item.key, item.value)
	}

	request.URL.RawQuery = q.Encode()

	// request.Header.Add("access-control-allow-headers", "*")
	// request.Header.Add("access-control-allow-origin", "*")
	// request.Header.Add("accept", "application/json, text/plain, */*")
	// request.Header.Add("Content-Type", "application/json; charset=utf-8")

	for _, item := range c.headers {
		request.Header.Add(item.key, item.value)
	}

	client := http.Client{
		Timeout: time.Second * 30,
	}

	response, err := client.Do(request)

	if err != nil {
		return nil, err
	}

	return response, nil
}

func (c *client) Decode(decode any) (*http.Response, error) {
	res, err := c.Exec()
	if err != nil {
		return nil, err
	}

	data, err := io.ReadAll(res.Body)

	if err != nil {
		return nil, err
	}

	if decode != nil {
		if err := json.Unmarshal(data, decode); err != nil {
			return nil, err
		}
	}

	return res, nil
}

func Request(url string, method string) *client {
	return &client{
		Url:    url,
		method: method,
	}
}

func Post(url string) *client {
	return &client{
		Url:    url,
		method: http.MethodPost,
	}
}

func Get(url string) *client {
	return &client{
		Url:    url,
		method: http.MethodGet,
	}
}

func Patch(url string) *client {
	return &client{
		Url:    url,
		method: http.MethodPatch,
	}
}

func Delete(url string) *client {
	return &client{
		Url:    url,
		method: http.MethodDelete,
	}
}

func Put(url string) *client {
	return &client{
		Url:    url,
		method: http.MethodPut,
	}
}
