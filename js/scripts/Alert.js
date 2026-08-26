import React from 'react'

export default class Alert extends React.Component {
    render() {
        return this.props.error ? (
            <div className="kp-login-alert kp-login-alert-error" role="alert">
                {this.props.error}
            </div>
        ) : null
    }
}
