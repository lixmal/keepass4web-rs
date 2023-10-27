import React from 'react'
import Classnames from 'classnames'

import withNavigateHook from './nagivateHook'


class NodeViewer extends React.Component {
    constructor(props) {
        super(props)
        this.setHide = this.setHide.bind(this)
    }

    showTooltip(btn, message) {
        $(btn).tooltip('hide')
            .attr('data-original-title', message)
            .tooltip('show')
        setTimeout(function () {
            $(btn).tooltip('destroy');
        }, 1000);
    }

    setHide(target, hide, name, data) {
        let entry = this.props.entry
        if (!entry) return
        // TODO: Handle empty passwords
        // These are currently kept as ***** without indication to the user
        if (typeof data === 'undefined')
            data = null
        if (!name || name === 'password')
            entry.password = data
        else if (entry.strings)
            entry.strings[name] = data

        if (hide === true)
            target.childNodes[0].className = 'glyphicon glyphicon-eye-open'
        else
            target.childNodes[0].className = 'glyphicon glyphicon-eye-close'

        this.forceUpdate()
    }

    PWHandler(name, event) {
        let target = event.currentTarget
        if (target.childNodes[0].className == 'glyphicon glyphicon-eye-close') {
            this.setHide(target, true, name)
            return
        }

        // else true:
        event.persist()
        this.serverRequest = KeePass4Web.ajax('get_protected', {
            method: 'GET',
            data: {
                entry_id: this.props.entry.id,
                name: name
            },
            success: function (data) {
                this.setHide(target, false, name, data.data)

                // hide password after X seconds
                setTimeout(this.PWTimeout.bind(this, target, name), this.props.timeoutSec)
            }.bind(this),
            error: KeePass4Web.error.bind(this),
        })

    }

    copyHandler(value, event) {
        let btn = event.currentTarget
        if (value == null)
            value = ''
        navigator.clipboard.writeText(value).then(
            function () {
                this.showTooltip(btn, 'Copied')
            }.bind(this),
            function () {
                this.showTooltip(btn, 'Failed to copy')
            }.bind(this),
        )
    }

    copyPWHandler(name, event) {
        let target = event.currentTarget.previousSibling

        this.serverRequest = KeePass4Web.ajax('get_protected', {
            method: 'GET',
            data: {
                entry_id: this.props.entry.id,
                name: name
            },
            // need same thread here, else copy won't work by browser restrictions
            async: false,
            success: function (data) {
                this.copyHandler(data.data, event)
            }.bind(this),
            error: KeePass4Web.error.bind(this),
        })
        this.setHide(target, true, name)
    }

    downloadHandler(filename, event) {
        // mostly taken from http://stackoverflow.com/questions/16086162/handle-file-download-from-ajax-post
        // with adjustments
        let xhr = new XMLHttpRequest()
        xhr.open('GET', 'get_file', true)
        xhr.responseType = 'arraybuffer'
        xhr.setRequestHeader('X-CSRF-Token', KeePass4Web.getCSRFToken())
        xhr.onload = function () {
            if (this.status === 200) {
                let filename = ""
                let disposition = xhr.getResponseHeader('Content-Disposition')
                if (disposition && disposition.indexOf('attachment') !== -1) {
                    let filenameRegex = /filename[^;=\n]*=((['"]).*?\2|[^;\n]*)/
                    let matches = filenameRegex.exec(disposition)
                    if (matches != null && matches[1]) {
                        filename = decodeURIComponent(matches[1].replace(/['"]/g, '').replace(/^UTF-8/i, ''))
                    }
                }
                let type = xhr.getResponseHeader('Content-Type')

                let blob = new Blob([this.response], {type: type})
                if (typeof window.navigator.msSaveBlob !== 'undefined') {
                    window.navigator.msSaveBlob(blob, filename)
                } else {
                    let URL = window.URL || window.webkitURL
                    let downloadUrl = URL.createObjectURL(blob)

                    if (filename) {
                        let a = document.createElement("a")
                        if (typeof a.download === 'undefined') {
                            window.location = downloadUrl
                        } else {
                            a.href = downloadUrl
                            a.download = filename
                            document.body.appendChild(a)
                            a.click()
                        }
                    } else {
                        window.location = downloadUrl
                    }

                    setTimeout(function () {
                        URL.revokeObjectURL(downloadUrl)
                    }, 100)
                }
            } else if (this.status >= 400) {
                KeePass4Web.error(xhr, null, xhr.responseText)
            }
        }
        xhr.setRequestHeader('Content-type', 'application/x-www-form-urlencoded; charset=UTF-8')
        xhr.setRequestHeader('X-Requested-With', 'XMLHttpRequest')

        KeePass4Web.restartTimer(true)

        xhr.send('id=' + encodeURIComponent(this.props.entry.id) + '&filename=' + encodeURIComponent(filename))
    }

    PWTimeout(target, name) {
        // ignore hidden passwords
        if (target.textContent === true) return
        this.setHide(target, true, name)
    }

    componentWillUnmount() {
        if (this.serverRequest)
            this.serverRequest.abort()
    }

    render() {
        let classes = Classnames({
            'panel': true,
            'panel-default': true,
            'loading-mask': this.props.mask,
        })

        if (!this.props.entry) return (<div className={classes}></div>)

        let entry = this.props.entry

        let fields = []
        let strings = entry.strings
        if (strings) {
            fields.push(
                <tr key="fields-header">
                    <th className="kp-fields" colSpan="3">Fields</th>
                </tr>
            )
        }
        for (let string in strings) {
            if (strings.hasOwnProperty(string)) {
                fields.push(
                    <tr key={string}>
                        <td className="kp-wrap">{string}</td>
                        <td className="kp-wrap">
                            {
                                entry.protected && entry.protected.hasOwnProperty(string) ?
                                    (strings[string] == null ? '******' : strings[string])
                                    : strings[string]
                            }
                        </td>
                        {entry.protected && entry.protected.hasOwnProperty(string) ?
                            <td>
                                <div className="btn-group" role="group">
                                    <button
                                        onClick={this.PWHandler.bind(this, string)}
                                        type="button"
                                        className="btn btn-default btn-sm"
                                    >
                                        <span className="glyphicon glyphicon-eye-open"></span>
                                    </button>
                                    <button
                                        onClick={this.copyPWHandler.bind(this, string)}
                                        type="button"
                                        className="btn btn-default btn-sm"
                                    >
                                        <span className="glyphicon glyphicon-copy"></span>
                                    </button>
                                </div>
                            </td>
                            : <td></td>}
                    </tr>
                )
            }
        }

        let files = []
        let binary = entry.binary
        if (binary) {
            files.push(
                <tr key="files-header">
                    <th className="kp-files" colSpan="3">Files</th>
                </tr>
            )
        }
        for (let file in binary) {
            if (binary.hasOwnProperty(file)) {
                files.push(
                    <tr key={file}>
                        <td colSpan="2" className="kp-wrap">
                            {file}
                        </td>
                        <td>
                            <button
                                onClick={this.downloadHandler.bind(this, file)}
                                type="button"
                                className="btn btn-default btn-sm"
                            >
                                <span className="glyphicon glyphicon-download-alt"></span>
                            </button>
                        </td>
                    </tr>
                )
            }
        }


        let icon = null
        if (entry.custom_icon_uuid)
            icon = <img className="kp-icon" src={'api/v1/icon/' + encodeURIComponent(entry.custom_icon_uuid)}/>
        else if (entry.icon)
            icon = <img className="kp-icon" src={'assets/img/icons/' + encodeURIComponent(entry.icon) + '.png'}/>

        let tags = []
        for (let tag in entry.tags) {
            tags.push(
                <span key={tag} className="kp-wrap badge badge-pill badge-light">{entry.tags[tag]}</span>
            )
        }

        return (
            <div className={classes}>
                <div className="panel-heading">
                    {icon}
                    {entry.title}
                </div>
                <div className="panel-body">
                    <table className="table table-hover table-condensed kp-table">
                        <colgroup>
                            <col className="kp-entry-label"/>
                            <col className="kp-entry-value"/>
                            <col className="kp-entry-buttons"/>
                        </colgroup>
                        <tbody>
                        <tr>
                            <td className="kp-wrap">
                                Username
                            </td>
                            <td className="kp-wrap">
                                {entry.username}
                            </td>
                            <td>
                                <button
                                    onClick={this.copyHandler.bind(this, entry.username)}
                                    type="button"
                                    className="btn btn-default btn-sm"
                                >
                                    <span className="glyphicon glyphicon-copy"></span>
                                </button>
                            </td>
                        </tr>
                        <tr>
                            <td className="kp-wrap">
                                Password
                            </td>
                            <td className="kp-wrap">
                                {entry.password == null ? '******' : entry.password}
                            </td>
                            <td>
                                <div className="btn-group" role="group">
                                    <button
                                        onClick={this.PWHandler.bind(this, 'password')}
                                        type="button"
                                        className="btn btn-default btn-sm"
                                    >
                                        <span className="glyphicon glyphicon-eye-open"></span>
                                    </button>
                                    <button
                                        onClick={this.copyPWHandler.bind(this, 'password')}
                                        type="button"
                                        className="btn btn-default btn-sm"
                                    >
                                        <span className="glyphicon glyphicon-copy"></span>
                                    </button>
                                </div>
                            </td>
                        </tr>
                        <tr>
                            <td>
                                URL
                            </td>
                            <td className="kp-wrap">
                                <a href={entry.url} target="_blank" rel="noopener noreferrer">{entry.url}</a>
                            </td>
                            <td>
                            </td>
                        </tr>
                        <tr>
                            <td className="kp-wrap">
                                Notes
                            </td>
                            <td className="kp-wrap-comment">
                                {entry.notes}
                            </td>
                            <td>
                            </td>
                        </tr>
                        <tr>
                            <td className="kp-wrap">
                                Tags
                            </td>
                            <td className="kp-wrap">
                                {tags}
                            </td>
                            <td>
                            </td>
                        </tr>
                        {fields}
                        {files}
                        </tbody>
                    </table>
                </div>
            </div>
        )
    }
}

export default withNavigateHook(NodeViewer)
