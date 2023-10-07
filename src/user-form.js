export const get_local_user_form_errors = (formData) => {
    const username = formData.get('username');
    const password = formData.get('password');
    const repassword = formData.get('repassword');

    let errors = { repassword_error: '', password_error: '', username_error: '' };
    if(password != '' && repassword != '' && password != repassword) {
        errors.repassword_error = 'Must match';
    }
    if(password == null || password != null && password.toString().length < 6) {
        errors.password_error= 'Must be at least 6 characters';
    }
    if(password != null && password.toString().length > 128) {
        errors.password_error= 'Must be at most 128 characters';
    }
    if(username != null && !username.toString().match(/^[0-9a-zA-Z]+$/)) {
        errors.username_error= 'Must be alphanumeric';
    }
    if(username == null || username != null && username.toString() == '') {
        errors.username_error= 'Cannot be empty';
    }
    if(username != null && username.toString().length > 32) {
        errors.username_error= 'Must be at most 32 characters';
    }
    return errors;
};